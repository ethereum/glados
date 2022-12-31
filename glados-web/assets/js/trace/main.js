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
    blue: 0,
    orange: 1,
    red: 2,
    other: 5,
    green: 4,
    brown: 8,
    gray: 9,

};

// Converts json response to format expected by D3 ForceGraph:
// { nodes: [{ id, group }], links: [{ source_id, target_id, group }] }
// Group of nodes determines color, group of links determines thickness.
function createGraphData(trace) {

    if (Object.keys(trace).length === 0) {
        return {
            nodes: [{ id: "local", group: colors.orange, timestamp: 0 }],
            links: [],
        }
    }

    let successfulRoute = computeSuccessfulRoute(trace);

    console.log('Route:');
    console.log(successfulRoute);

    // Create nodes.
    let nodes = [];
    let nodesSeen = [];
    let responses = trace.responses;
    let trinNodesResponded = 0;
    let trinNodesNoResponse = 0;
    Object.keys(responses).forEach((enr, _) => {

        let node = responses[enr];
        let timestamp = node.timestamp_ms;
        let respondedWith = node.responded_with;
        if (!Array.isArray(respondedWith)) {
            return;
        }
        if (!nodesSeen.includes(enr)) {
            let group = 0;
            if ('origin' in trace && trace.origin == enr) {
                group = colors.orange;
            } else {
                if (ENR.ENR.decodeTxt(enr).client === 'trin') {

                    trinNodesResponded++;
                }
                if ('found_content_at' in trace && trace.found_content_at == enr) {
                    group = colors.green;
                } else if (respondedWith.length == 0) {
                    group = colors.brown;
                } else {
                    group = colors.blue;
                }
            }
            let metadata = trace.node_metadata[enr];
            let ip = metadata.ip;
            let port = metadata.port;
            let distance_to_content = metadata.distance_to_content;
            let distance_log2 = metadata.distance_log2;
            let node_id = metadata.node_id;
            let client = ENR.ENR.decodeTxt(enr).client;
            nodes.push({
                id: enr,
                group: group,
                timestamp: timestamp,
                ip: ip,
                port: port,
                distance: distance_to_content,
                distance_log2,
                node_id: node_id,
                client: client
            });
            nodesSeen.push(enr);
        }
    });

    // Create links.
    let links = [];
    Object.keys(responses).forEach((enr_source, _) => {

        let node = responses[enr_source];
        let responded_with = node.responded_with;
        if (!Array.isArray(responded_with)) {
            return;
        }
        responded_with.forEach((enr_target, _) => {
            if (!nodesSeen.includes(enr_target)) {
                if (ENR.ENR.decodeTxt(enr_target).client === 'trin') {
                    trinNodesNoResponse++;
                }
                let metadata = trace.node_metadata[enr_target];
                let ip = metadata.ip;
                let port = metadata.port;
                let distance_to_content = metadata.distance_to_content;
                let distance_log2 = metadata.distance_log2;
                let node_id = metadata.node_id;
                let client = ENR.ENR.decodeTxt(enr_target).client;
                nodes.push({
                    id: enr_target,
                    group: colors.gray,
                    ip: ip,
                    port: port,
                    distance: distance_to_content,
                    distance_log2,
                    node_id: node_id,
                    client: client
                });
                nodesSeen.push(enr_target)
            }
            let value = 1;
            if (successfulRoute.includes(enr_source)
                && successfulRoute.includes(enr_target)) {
                value = 40;
            }
            links.push({
                source: enr_source,
                target: enr_target,
                value: value
            })
        })
    });
    let graph = {
        nodes: nodes,
        links: links,
        metadata: {
            nodesContacted: nodes.length - 1,
            nodesResponded: Object.keys(responses).length - 1,
            trinNodesContacted: trinNodesResponded + trinNodesNoResponse,
            trinNodesResponded: trinNodesResponded,
        }
    }
    return graph;

}



// Returns a list of nodes in the route.
// Starts from the end (where the content was found) and finds the way back to the origin.
function computeSuccessfulRoute(trace) {

    if (!('origin' in trace && 'found_content_at' in trace)) {
        return [];
    }

    let origin = trace.origin;
    let found_at = trace.found_content_at;

    let route = [];
    // Find the node that contains found_at.
    let target_node = found_at;
    route.push(target_node);
    let route_info = trace.responses;
    while (target_node != origin) {

        let previous_target = target_node;
        Object.keys(route_info).forEach((node_id, _) => {

            let node = route_info[node_id];
            let responses = node.responded_with;

            // Find the node that responded with the current target node.
            if (Array.isArray(responses) && responses.includes(target_node)) {
                target_node = node_id;
                route.push(target_node);
            }
        })
        if (previous_target == target_node) {
            // Did not progress, no route found.
            return [];
        }
    }
    return route;

}

// Generates a string to appear on hover-over of a node.
function generateNodeMetadata(node) {

    let timestamp = node.timestamp;
    let client = node.client;
    let metadata = `${node.id}\n`;
    if (timestamp !== undefined) {
        metadata += `${timestamp} ms\n`;
    }
    if (client !== undefined) {
        metadata += `${client}`;
    }
    return metadata;

}

function generateTable(nodes) {

    nodes.sort((a, b) => a.distance - b.distance);
    nodes.forEach((node, index) => {

        let node_id_string = node.node_id;
        node_id_string = node_id_string.substr(0, 6)
            + '...' + node_id_string.substr(node_id_string.length - 4, node_id_string.length);

        let enr_shortened = node.id.substr(5, 4) + '...' + node.id.substr(node.id.length - 5, node.id.length);

        let tr = document.createElement("tr");
        tr.innerHTML = `<th scope="row">${index + 1}</th>
            <td>${enr_shortened}</td>
            <td>${node_id_string}</td>
            <td>${node.distance_log2}</td>
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
    let enr = node.target.__data__.id.substring(4);
    let id_string = '#' + enr;
    $(id_string).css('background-color', 'lightgray');
}

function highlightNode(node) {
    d3.selectAll("g").selectAll("circle")
        .filter(d => d.id === node)
        .attr("r", function (node) {
            return 10;
        });
}

function highlightTrinNodes() {

    d3.selectAll("g").selectAll("circle")
        .attr("r", function (node) {
            if (node.client === "trin") {
                return 5;
            } else {
                return 0;
            }

        });

}

function highlightTrinNodes() {

    d3.selectAll("g").selectAll("circle")
        .attr("r", function (node) {
            let enr = node.id;
            let client = ENR.ENR.decodeTxt(enr).client;
            if (client === "trin") {
                return 5;
            } else {
                return 0;
            }

        });

}

function unHighlight() {

    d3.selectAll("g").selectAll("circle")
        .attr("r", function (node) {
            return 5;
        });
    $('tr').css('background-color', 'white');

}
