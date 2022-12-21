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
    Object.keys(responses).forEach((node_id, _) => {

        let node = responses[node_id];
        let timestamp = node.timestamp_ms;
        let respondedWith = node.responded_with;
        if (!Array.isArray(respondedWith)) {
            return;
        }
        if (!nodesSeen.includes(node_id)) {
            let group = 0;
            if ('origin' in trace && trace.origin == node_id) {
                group = colors.orange;
            } else {
                if ('received_content_from_node' in trace && trace.received_content_from_node == node_id) {
                    group = colors.green;
                } else if (respondedWith.length == 0) {
                    group = colors.brown;
                } else {
                    group = colors.blue;
                }
            }
            let metadata = trace.node_metadata[node_id];
            let enr = metadata.enr;
            let ip = metadata.ip;
            let port = metadata.port;
            let distance_to_content = metadata.distance_to_content;
            let distance_log2 = metadata.distance_log2;
            let node_id_full = metadata.node_id;
            let client = ENR.ENR.decodeTxt(enr).client;
            if (client === 'trin') { trinNodesResponded++ }
            nodes.push({
                id: node_id,
                enr: enr,
                group: group,
                timestamp: timestamp,
                ip: ip,
                port: port,
                distance: distance_to_content,
                distance_log2,
                node_id: node_id_full,
                client: client
            });
            nodesSeen.push(node_id);
        }
    });

    // Create links.
    let links = [];
    Object.keys(responses).forEach((node_id_source, _) => {

        let node = responses[node_id_source];
        let responded_with = node.responded_with;
        if (!Array.isArray(responded_with)) {
            return;
        }
        responded_with.forEach((node_id_target, _) => {
            if (!nodesSeen.includes(node_id_target)) {
                let metadata = trace.node_metadata[node_id_target];
                let enr = metadata.enr;
                let ip = metadata.ip;
                let port = metadata.port;
                let distance_to_content = metadata.distance_to_content;
                let distance_log2 = metadata.distance_log2;
                let node_id = metadata.node_id;
                let client = ENR.ENR.decodeTxt(enr).client;
                if (client === 'trin') { trinNodesNoResponse++ }
                nodes.push({
                    id: node_id_target,
                    enr: enr,
                    group: colors.gray,
                    ip: ip,
                    port: port,
                    distance: distance_to_content,
                    distance_log2,
                    node_id: node_id,
                    client: client
                });
                nodesSeen.push(node_id_target)
            }
            let value = 1;
            if (successfulRoute.includes(node_id_source)
                && successfulRoute.includes(node_id_target)) {
                value = 40;
            }
            links.push({
                source: node_id_source,
                target: node_id_target,
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

    if (!('origin' in trace && 'received_content_from_node' in trace)) {
        return [];
    }

    let origin = trace.origin;
    let found_at = trace.received_content_from_node;

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

        let node_id_string = node.id;
        node_id_string = node_id_string.substr(0, 6)
            + '...' + node_id_string.substr(node_id_string.length - 4, node_id_string.length);

        let enr_shortened = node.enr.substr(5, 4) + '...' + node.id.substr(node.id.length - 5, node.id.length);

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
