// Creates a graph from a trace object and returns an SVG.
function createGraph(graphData) {

    return ForceGraph(graphData, {
        nodeId: d => d.id,
        nodeGroup: d => d.group,
        nodeGroups: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        nodeTitle: d => generateNodeMetadata(d),
        linkStrokeWidth: l => Math.sqrt(l.value),
        width: $('#graph').width(),
        height: $('#graph').height(),
        invalidation: null,
    });

}

const colors = {
    blue   : 0,
    orange : 1,
    red    : 2,
    green  : 4,
    yellow : 5,
    brown  : 8,
    gray   : 9,
};

// Converts json response to format expected by D3 ForceGraph:
// { nodes: [{ id, group }], links: [{ source_id, target_id, group }] }
// Group of nodes determines color, group of links determines thickness.
function createGraphData(trace) {

    if (Object.keys(trace).length === 0) {
        return {
            nodes: [{ id: "local", group: colors.orange, durationMs: 0 }],
            links: [],
        }
    }

    let successfulRoute = computeSuccessfulRoute(trace);

    console.log('Route:');
    console.log(successfulRoute);

    let metadata = {};
    Object.keys(trace.metadata).forEach((nodeId) => {
      let meta = trace.metadata[nodeId];
      let enr = meta.enr;
      let decodedEnr = ENR.ENR.decodeTxt(enr).enr;

      let ip = decodedEnr.ip || "localhost"; // Edge case for local node
      let port = decodedEnr.udp;
      let distance = BigInt(meta.distance);
      let distanceLog2 = bigLog2(distance);
      let client = decodedEnr.client;

      metadata[nodeId] = {
        enr,
        ip,
        port,
        distance,
        distanceLog2,
        client
      }
    });

    // Create nodes.
    let nodes = [];
    let nodesSeen = [];
    let responses = trace.responses;
    Object.keys(responses).forEach((nodeId, _) => {

        let node = responses[nodeId];
        let durationMs = node.durationMs;
        let respondedWith = node.respondedWith;
        if (!Array.isArray(respondedWith)) {
            return;
        }
        if (!nodesSeen.includes(nodeId)) {
            let group = 0;
            if ('origin' in trace && trace.origin == nodeId) {
                group = colors.orange;
            } else {
                if ('receivedFrom' in trace && trace.receivedFrom == nodeId) {
                    group = colors.green;
                } else if (respondedWith.length == 0) {
                    group = colors.brown;
                } else if (trace.cancelled.includes(nodeId)) {
                    group = colors.yellow;
                } else {
                    group = colors.blue;
                }
            }
            nodes.push({
                id: nodeId,
                group,
                durationMs,
                ...metadata[nodeId]
            });
            nodesSeen.push(nodeId);
        }
    });

    // Create links.
    let links = [];
    Object.keys(responses).forEach((nodeIdSource, _) => {

        let node = responses[nodeIdSource];
        let respondedWith = node.respondedWith;
        if (!Array.isArray(respondedWith)) {
            return;
        }
        respondedWith.forEach((nodeIdTarget, _) => {
            if (!nodesSeen.includes(nodeIdTarget)) {
                let group = colors.gray;

                if (trace.cancelled.includes(nodeIdTarget)) {
                  group = colors.yellow
                }

                nodes.push({
                    id: nodeIdTarget,
                    group: group,
                    ...metadata[nodeIdTarget]
                });
                nodesSeen.push(nodeIdTarget)
            }
            let value = 1;
            if (successfulRoute.includes(nodeIdSource)
                && successfulRoute.includes(nodeIdTarget)) {
                value = 40;
            }
            links.push({
                source: nodeIdSource,
                target: nodeIdTarget,
                value: value
            })
        })
    });
    let graph = {
        nodes: nodes,
        links: links,
        metadata: {
            nodesContacted: nodes.length - 1,
            nodesResponded: Object.keys(responses).length - 1
        }
    }
    return graph;

}

// Returns a list of nodes in the route.
// Starts from the end (where the content was found) and finds the way back to the origin.
function computeSuccessfulRoute(trace) {

    if (!('origin' in trace && 'receivedFrom' in trace)) {
        return [];
    }

    let origin = trace.origin;
    let receivedFrom = trace.receivedFrom;

    let route = [];
    route.push(receivedFrom);
    let route_info = trace.responses;
    while (receivedFrom != origin) {

        let previous_target = receivedFrom;
        Object.keys(route_info).forEach((nodeId, _) => {

            let node = route_info[nodeId];
            let responses = node.respondedWith;

            // Find the node that responded with the current target node.
            if (Array.isArray(responses) && responses.includes(receivedFrom)) {
                receivedFrom = nodeId;
                route.push(receivedFrom);
            }
        })
        if (previous_target == receivedFrom) {
            // Did not progress, no route found.
            return [];
        }
    }
    return route;

}

// Generates a string to appear on hover-over of a node.
function generateNodeMetadata(node) {

    let durationMs = node.durationMs;
    let client = node.client;
    let metadata = `${node.id}\n`;
    if (durationMs !== undefined) {
        metadata += `${durationMs} ms\n`;
    }
    if (client !== undefined) {
        metadata += `${client}`;
    }
    return metadata;

}

function generateTable(nodes) {
    nodes.sort((a, b) => a.distance < b.distance ? -1 : (a.distance > b.distance) ? 1 : 0);
    nodes.forEach((node, index) => {

        let nodeIdString = node.id;
        nodeIdString = nodeIdString.substr(0, 6)
            + '...' + nodeIdString.substr(nodeIdString.length - 4, nodeIdString.length);

        let enr_shortened = node.enr.substr(5, 4) + '...' + node.id.substr(node.id.length - 5, node.id.length);

        let tr = document.createElement("tr");
        tr.innerHTML = `<th scope="row">${index + 1}</th>
            <td>${enr_shortened}</td>
            <td>${nodeIdString}</td>
            <td>${node.distanceLog2}</td>
            <td>${node.ip}:${node.port}</td>
            <td>${node.client === undefined ? "" : node.client}</td>`;

        tr.addEventListener('mouseenter', () => {
            tr.style.backgroundColor = 'lightgray';
            highlightNode(node.id);
        });
        tr.addEventListener('mouseleave', () => {
            tr.style.backgroundColor = 'white';
            unHighlight();
        });
        tr.id = node.id.substring(4);

        $('#enr-table').append(tr);
    });

}

function highlightTableEntry(node) {
    unHighlight();
    let enr = node.target.__data__.id;
    let enr_substr = enr.substring(4);
    let id_string = '#' + enr_substr;
    $(id_string).css('background-color', 'lightgray');

    var element = document.getElementById(enr_substr);
    element.scrollIntoView({ block: "nearest", behavior: "auto" });

    highlightNode(enr);
}

function highlightNode(node) {
    d3.selectAll("g").selectAll("circle")
        .filter(d => d.id === node)
        .attr("r", function (node) {
            return 10;
        });
}

function unHighlight() {

    d3.selectAll("g").selectAll("circle")
        .attr("r", function (node) {
            return 5;
        });
    $('tr').css('background-color', 'white');

}

function bigLog2(num) {
  const one = BigInt(1);
  let ret = BigInt(0);

  while (num>>=one) ret++

  return ret;
}

function supportsTrace(client) {
  client === 'trin' | client === 'fluffy'
}

