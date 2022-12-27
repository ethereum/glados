// Creates a graph from a trace object and returns an SVG.
function createGraph(graphData) {

    return ForceGraph(graphData, {
        nodeId: d => d.id,
        nodeGroup: d => d.group,
        nodeGroups: [0, 1, 2, 3, 4, 5, 6, 7, 9],
        nodeTitle: d => generateNodeMetadata(d),
        linkStrokeWidth: l => Math.sqrt(l.value),
        width: $('#graph').width(),
        height: $('#graph').height(),
        invalidation: null,
    });

}

// Converts json response to format expected by D3 ForceGraph:
// { nodes: [{ id, group }], links: [{ source_id, target_id, group }] }
// Group of nodes determines color, group of links determines thickness.
function createGraphData(trace) {

    const colors = {
        blue: 0,
        orange: 1,
        red: 2,
        green: 4,
        brown: 9,
        gray: 8,

    };

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
            console.log(trace.node_metadata[enr].distance_to_content);
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
            let scale = trace.node_metadata[enr].distance_to_content / 10000;
            nodes.push({ id: enr, group: group, timestamp: timestamp, x: scale, y: timestamp });
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
                let scale = trace.node_metadata[enr_target].distance_to_content / 10000;
                nodes.push({ id: enr_target, group: colors.gray, x: scale, y: node.timestamp_ms + 30 });
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

    let enr = ENR.ENR.decodeTxt(node.id);
    let timestamp = node.timestamp;
    let client = enr.client;
    let metadata = `${node.id}\n`;
    if (timestamp !== undefined) {
        metadata += `${timestamp} ms\n`;
    }
    if (client !== undefined) {
        metadata += `${client}`;
    }
    return metadata;

}

