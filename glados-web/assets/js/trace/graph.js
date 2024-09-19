// Copyright 2021 Observable, Inc.
// Released under the ISC license.
// https://observablehq.com/@d3/force-directed-graph
function ForceGraph({
    nodes, // an iterable of node objects (typically [{id}, …])
    links // an iterable of link objects (typically [{source, target}, …])
}, {
    nodeId = d => d.id, // given d in nodes, returns a unique identifier (string)
    nodeGroup, // given d in nodes, returns an (ordinal) value for color
    nodeGroups, // an array of ordinal values representing the node groups
    nodeTitle, // given d in nodes, a title string
    nodeFill = "currentColor", // node stroke fill (if not using a group color encoding)
    nodeStroke = "#fff", // node stroke color
    nodeStrokeWidth = 1.5, // node stroke width, in pixels
    nodeStrokeOpacity = 1, // node stroke opacity
    nodeRadius = 5, // node radius, in pixels
    nodeStrength,
    linkSource = ({ source }) => source, // given d in links, returns a node identifier string
    linkTarget = ({ target }) => target, // given d in links, returns a node identifier string
    linkStroke = "#999", // link stroke color
    linkStrokeOpacity = 0.6, // link stroke opacity
    linkStrokeWidth = 5, // given d in links, returns a stroke width in pixels
    linkStrokeLinecap = "round", // link stroke linecap
    linkStrength,
    colors = d3.schemeTableau10, // an array of color strings, for the node groups
    width = 640, // outer width,m in pixels
    height = 400, // outer height, in pixels
    invalidation, // when this promise resolves, stop the simulation
    contentId, // the content ID to highlight
    sortByNodeId = true,
} = {}) {
    // Compute values.
    const N = d3.map(nodes, nodeId).map(intern);
    const LS = d3.map(links, linkSource).map(intern);
    const LT = d3.map(links, linkTarget).map(intern);
    if (nodeTitle === undefined) nodeTitle = (_, i) => N[i];
    const T = nodeTitle == null ? null : d3.map(nodes, nodeTitle);
    const G = nodeGroup == null ? null : d3.map(nodes, nodeGroup).map(intern);
    const W = typeof linkStrokeWidth !== "function" ? null : d3.map(links, linkStrokeWidth);
    const L = typeof linkStroke !== "function" ? null : d3.map(links, linkStroke);

    // Replace the input nodes and links with mutable objects for the simulation.
    if (sortByNodeId) {
        nodes = d3.map(nodes, (node, i) => ({ id: N[i], fixedX: (calculateNodeIdX(node.id) * width) - (width / 2), ...node}));
    } else {
        nodes = d3.map(nodes, (_, i) => ({ id: N[i] }));
    }
    links = d3.map(links, (_, i) => ({ source: LS[i], target: LT[i] }));

    // Compute default domains.
    if (G && nodeGroups === undefined) nodeGroups = d3.sort(G);

    // Construct the scales.
    const color = nodeGroup == null ? null : d3.scaleOrdinal(nodeGroups, colors);

    // Construct the forces.
    const forceNode = d3.forceManyBody();
    const forceLink = d3.forceLink(links).id(({ index: i }) => N[i]);
    if (nodeStrength !== undefined) forceNode.strength(nodeStrength);
    if (linkStrength !== undefined) forceLink.strength(linkStrength);

    const paddingY = 50;
    const xPadding = 0;
    let simulation;

    if (sortByNodeId) {
        simulation = d3.forceSimulation(nodes)
            .force("link", forceLink)
            .force("charge", forceNode.strength(-300)) // Reduce the strength of repulsion
            .force("x", d3.forceX(d => d.fixedX).strength(1))
            .force("collide", d3.forceCollide(nodeRadius * 1.2))
            .force("boundary", forceBoundary(width, height, paddingY, 1)) // Add the boundary force
            .on("tick", ticked);
    } else {
        simulation = d3.forceSimulation(nodes)
            .force("link", forceLink)
            .force("charge", forceNode)
            .force("center", d3.forceCenter())
            .on("tick", ticked);
    }

    const svg = d3.create("svg")
        .attr("width", width)
        .attr("height", height)
        .attr("viewBox", [-width / 2, -height / 2, width, height])
        .attr("style", "max-width: 100%; height: auto; height: intrinsic;");

    if (sortByNodeId) {
        // Add the vertical dotted line
        const contentIdMarkerX = calculateNodeIdX(contentId) * width;
        console.log(contentIdMarkerX);
        svg.append("line")
            .attr("x1", contentIdMarkerX - (width / 2))
            .attr("y1", -height / 2)
            .attr("x2", contentIdMarkerX - (width / 2))
            .attr("y2", height / 2)
            .attr("stroke", "black")  
            .attr("stroke-width", 1)  
            .attr("stroke-dasharray", "5,5");
    }
    
    svg.append("line")
        .attr("x1",  -(width / 2) + xPadding)
        .attr("y1", -height / 2)
        .attr("x2", -(width / 2) + xPadding)
        .attr("y2", height / 2)
        .attr("stroke", "black")  
        .attr("stroke-width", 1)  
        .attr("stroke-dasharray", "5,5");

    svg.append("line")
        .attr("x1",  (width / 2) - xPadding)
        .attr("y1", -height / 2)
        .attr("x2", (width / 2) - xPadding)
        .attr("y2", height / 2)
        .attr("stroke", "black")  
        .attr("stroke-width", 1)  
        .attr("stroke-dasharray", "5,5");

    const link = svg.append("g")
        .attr("stroke", typeof linkStroke !== "function" ? linkStroke : null)
        .attr("stroke-opacity", linkStrokeOpacity)
        .attr("stroke-width", typeof linkStrokeWidth !== "function" ? linkStrokeWidth : null)
        .attr("stroke-linecap", linkStrokeLinecap)
        .selectAll("line")
        .data(links)
        .join("line");

    const node = svg.append("g")
        .attr("fill", nodeFill)
        .attr("stroke", nodeStroke)
        .attr("stroke-opacity", nodeStrokeOpacity)
        .attr("stroke-width", nodeStrokeWidth)
        .selectAll("circle")
        .data(nodes)
        .join("circle")
        .attr("r", nodeRadius)
        .on("mouseenter", highlightTableEntry)
        .on("mouseleave", unHighlight)
        .call(drag(simulation));

    if (W) link.attr("stroke-width", ({ index: i }) => W[i]);
    if (L) link.attr("stroke", ({ index: i }) => L[i]);
    if (G) node.attr("fill", ({ index: i }) => color(G[i]));
    if (T) node.append("title").text(({ index: i }) => T[i]);
    if (invalidation != null) invalidation.then(() => simulation.stop());

    function intern(value) {
        return value !== null && typeof value === "object" ? value.valueOf() : value;
    }

    function ticked() {
        if (sortByNodeId) {
            link
                .attr("x1", d => d.source.fixedX)
                .attr("y1", d => d.source.y)
                .attr("x2", d => d.target.fixedX)
                .attr("y2", d => d.target.y);

            node
                .attr("cx", d => d.fixedX)
                .attr("cy", d => d.y);
        } else {
            const width = $('#graph').width();
            const height = $('#graph').height();
            link
                .attr("x1", d => enforceBorder(d.source.x, -width / 2, width / 2))
                .attr("y1", d => enforceBorder(d.source.y, -height / 2, (height / 2) - 24))
                .attr("x2", d => enforceBorder(d.target.x, -width / 2, width / 2))
                .attr("y2", d => enforceBorder(d.target.y, -height / 2, (height / 2) - 24));

            node
                .attr("cx", d => enforceBorder(d.x, -width / 2, width / 2))
                .attr("cy", d => enforceBorder(d.y, -height / 2, (height / 2) - 24));
        }
    }

    function drag(simulation) {
        const dragstarted = event => {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            event.subject.fx = sortByNodeId ? event.subject.fixedX : event.x;
            event.subject.fy = event.y;
        };

        const dragged = event => {
            event.subject.fx = sortByNodeId ? event.subject.fixedX : event.x;
            event.subject.fy = event.y;
        };

        function dragended(event) {
            if (!event.active) simulation.alphaTarget(0);
            event.subject.fx = null;
            event.subject.fy = null;
        }

        return d3.drag()
            .on("start", dragstarted)
            .on("drag", dragged)
            .on("end", dragended);
    }

    return Object.assign(svg.node(), { scales: { color } });
}

function forceBoundary(width, height, padding, strength = 0.05) {
    let nodes;
    function force(alpha) {
        const adjustedStrength = strength * alpha;
        for (const node of nodes) {
            if (node.x < -width/2 + padding) node.vx += ((-width/2 + padding) - node.x) * adjustedStrength;
            if (node.x > width/2 - padding) node.vx += ((width/2 - padding) - node.x) * adjustedStrength;
            if (node.y < -height/2 + padding) node.vy += ((-height/2 + padding) - node.y) * adjustedStrength;
            if (node.y > height/2 - padding) node.vy += ((height/2 - padding) - node.y) * adjustedStrength;
        }
    }

    force.initialize = (_) => nodes = _;

    return force;
}

function enforceBorder(position, lowerLimit, upperLimit) {
    lowerLimit += 20;
    upperLimit -= 20;
    if (position < lowerLimit) position = lowerLimit;
    if (position > upperLimit) position = upperLimit;
    return position;
}

function calculateNodeIdX(nodeId) {

    const nodeIdInt = BigInt(nodeId);
    const maxNodeId = BigInt("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    const nodeIdRatio = (Number(nodeIdInt.toString()) / Number(maxNodeId.toString()));
    return nodeIdRatio;
    
}
