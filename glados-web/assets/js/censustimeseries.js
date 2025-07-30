let daysAgo = 0;
let decodedEnrs = {};
let highlightedRowGroup = null;

function createSquareChart(width, data) {

    // Incoming data takes the following form.
    // nodeIdsWithNicknames and censuses.nodes are arrays of the same length and order.
    /* {
         "nodeIdsWithNicknames": [
           ["0x9891fe3fdfcec2707f877306fc6e5596b1405fedf70ad2911496c2ac40dfeda5", null],
           ["0x08653f55f8120591717aae0426c10c25f0e6a7db078c2e1516b4f3059cefff35", null],
           ["0xbefb246b22757267608a436d265018374592291f284c536f73c933b29a91e10a", "node-nickname"]
         ],
         "censuses": [
           {
             "censusId": 12345,
             "censusTime": "2023-12-13T02:03:38.389580Z",
             "nodes": [
               null,
               {
                 nodeEnrId: 345,
                 radiusAsPercentage: "5.03%",
                 client: {
                   slug: "trin",
                   name: "Trin",
                   color: "#9B59B6",
                 },
                 clientVersion: "0.1.2",
                 clientShortCommit: "0123abcd",
               }
               null,
               ...
             ]
           },
           ...
         ],
         "enrs" {
            345: "enr:-Jy4QOOcmr_MEnFj3-lNBg...ZiBcWbChOHq73nNxeu0ELYN1ZHCCIzE"
         }
       }*/

    // Create a combined array of node IDs and their info during each census.
    let nodesAndCensusInfos = zipNodesAndCensusData(data.nodeIdsWithNicknames, data.censuses, data.enrs);

    // Sort the combined array based on clientString and secondarily nodeId
    nodesAndCensusInfos.sort((a, b) => sortNodes(a, b));

    const x = d3.scaleTime()
        .domain(d3.extent(data.censuses, d => new Date(d.censusTime)))
        .range([0, width]);

    const nodes = nodesAndCensusInfos.map(d => d.nodeId);

    const cellHeight = 11;
    const height = cellHeight * nodes.length;
    const y = d3.scaleBand()
        .domain(nodes)
        .range([0, height])
        .padding(0.05);

    // Parameters for handling scaling of rows.
    const originalHeight = y.bandwidth();
    const expandedScaleFactor = 12.0;
    const expandedCellHeight = (originalHeight * expandedScaleFactor);

    // Create the SVG container.
    const marginTop = 110;
    const marginLeft = 30;
    const svg = d3.create("svg")
        .attr("width", width)
        .attr("height", height + expandedCellHeight + marginTop)
        .attr("viewBox", [-(width * 0.07), 0, width, height + expandedCellHeight + marginTop])
        .attr("overflow", "visible")
        .attr("style", "max-width: 90%; height: 100%; height: intrinsic;");


    const url = new URL(window.location);
    const subprotocol = url.searchParams.get('network');
    let title = data.censuses.length > 0 ? `${nodes.length} ${subprotocol} nodes found during 24 hour period beginning at ${data.censuses[0].censusTime}`
        : `No ${subprotocol} censuses found during this 24 hour period.`;

    // Append the title
    svg.append("text")
        .attr("x", width / 2)
        .attr("y", 30)
        .attr("text-anchor", "middle")
        .style("font-size", "24px")
        .text(title);

    // Append the previous button
    svg.append("a")
        .attr("xlink:href", "#")
        .on("click", function (event) {
            event.preventDefault();
            console.log(`days ago: ${daysAgo}`);
            censusTimeSeriesChart(daysAgo + 1);
            daysAgo++;
            console.log(`days ago now: ${daysAgo}`);
        }).append("text")
        .attr("x", (width / 2) - 130)
        .attr("y", 70)
        .text("< Previous");

    // Append the next button
    svg.append("a")
        .attr("xlink:href", "#")
        .on("click", function (event) {
            event.preventDefault();
            if (daysAgo == 0) {
                censusTimeSeriesChart(0);
                return;
            }
            console.log(`days ago: ${daysAgo}`);
            censusTimeSeriesChart(daysAgo - 1);
            daysAgo--;
            console.log(`days ago now: ${daysAgo}`);
        }).append("text")
        .attr("x", (width / 2) + 70)
        .attr("y", 70)
        .text("Next >");

    // Append X axis to the bottom and top
    svg.append("g")
        .attr("transform", `translate(0,${height + expandedCellHeight + marginTop - originalHeight})`)
        .attr("class", "x-axis")
        .call(d3.axisBottom(x))
        .selectAll("text")
        .style("font-size", "12px");
    svg.append("g")
        .attr("transform", `translate(0,${marginTop})`)
        .attr("class", "x-axis")
        .call(d3.axisTop(x))
        .selectAll("text")
        .style("font-size", "12px");

    // Create group for each row.
    const rowGroups = svg.selectAll(".row-group")
        .data(nodesAndCensusInfos)
        .enter()
        .append("g")
        .attr("class", "row-group")
        .attr("id", (d, i) => `row-${i}`);

    rowGroups.attr("transform", `translate(0, ${marginTop})`);

    data.censuses.forEach(census => {
        census.parsedTime = new Date(census.censusTime);
    });

    // Append squares to each row group.
    rowGroups.each(function (node, i) {
        node.censusInfos.forEach((censusInfo, j) => {

            const row = d3.select(this);
            const rect = row.append("rect")
                .attr("x", x(data.censuses[j].parsedTime))
                .attr("y", y(node.nodeId))
                .attr("width", `${(width * 0.96) / data.censuses.length}px`)
                .attr("height", y.bandwidth() + "px")
                .attr("stroke-width", 0.1)
                .attr("stroke", "rgb(245, 245, 245)")
                .attr("fill", censusInfo ? "green" : "gray");

            let title = "";
            if (censusInfo) {
                title = createHoverOverInfo(censusInfo, data.censuses[j].censusTime);
            } else {
                title = `Census started at ${data.censuses[j].censusTime}.\nNot found!`;
            }

            rect.append("title").text(title);

            rect.on("click", function (_, _) {
                window.open(`/census/?census-id=${data.censuses[j].censusId}`, '_blank');
            });

            rect.on("mouseenter", function (_) {
                rect.raise()
                    .style("stroke", "white")
                    .style("stroke-width", 4);
            }).on("mouseout", function () {
                d3.select(this)
                    .style("stroke", null)
                    .style("stroke-width", null);
            });
        });
    });

    // Append y-axis labels/link to each row group
    rowGroups.append("a")
        .attr("xlink:href", d => `/network/node/${d.nodeId}/`)
        .attr("target", "_blank")
        .append("text")
        .attr("x", -marginLeft)
        .attr("y", d => y(d.nodeId) + y.bandwidth() / 2)
        .attr("dy", ".35em")
        .attr("text-anchor", "end")
        .attr("fill", d => d.clientColor)
        .text(d => (d.nodeId.substring(0, 6) + '...' + d.nodeId.substring(d.nodeId.length - 4)));

    // Internal function to handle hover-over magnification effect.
    function highlightNode(node) {

        let hoveredIndex = nodes.indexOf(node.nodeId);

        let downwardShift = 0;

        // Remove existing client string labels if any.
        svg.selectAll(".client-string-label").remove();
        // Unhighlight the highlighted row.
        if (highlightedRowGroup) {
            highlightedRowGroup.attr("transform", null)
                .selectAll("rect")
                .attr("height", originalHeight);
            highlightedRowGroup.selectAll("text")
                .style("font-size", null) // Reset font size
                .attr("y", d => y(d.nodeId) + originalHeight / 2);
        }

        rowGroups.each(function (_, index) {
            const group = d3.select(this);
            let scaleFactor, heightIncrease;

            if (index === hoveredIndex) {
                scaleFactor = expandedScaleFactor;
                // Adjust the height of the row
                group.selectAll("rect")
                    .attr("height", expandedCellHeight);
                group.selectAll("text").style("font-size", "1.7em");
            } else {
                scaleFactor = 1;
            }

            heightIncrease = (originalHeight * scaleFactor) - originalHeight;
            downwardShift += heightIncrease;

            // Adjust the font size and position of the text
            group.selectAll("text")
                .attr("y", y(nodesAndCensusInfos[index].nodeId) + (originalHeight * scaleFactor) / 2);

            // Shift rows below the hovered row downwards
            group.attr("transform", `translate(0, ${marginTop + (downwardShift - heightIncrease)})`);

            // Append metadata
            if (index === hoveredIndex) {
                const yPos = y(nodesAndCensusInfos[index].nodeId) + (originalHeight * scaleFactor) / 2;

                // Append a new text element for nickname and clientString
                group.append("text")
                    .attr("class", "client-string-label")
                    .attr("x", -200)
                    .attr("y", yPos + -20)
                    .attr("text-anchor", "start")
                    .attr("fill", nodesAndCensusInfos[index].clientColor)
                    .text(nodesAndCensusInfos[index].nodeNickName);
                group.append("text")
                    .attr("class", "client-string-label")
                    .attr("x", -200)
                    .attr("y", yPos + 30)
                    .attr("text-anchor", "start")
                    .attr("fill", nodesAndCensusInfos[index].clientColor)
                    .text(nodesAndCensusInfos[index].clientString);
            }

        });

        highlightedRowGroup = rowGroups.filter(d => d.nodeId === node.nodeId);
    }

    // Apply hover effect to the row groups
    rowGroups.on("mouseenter", function (_, node) {
        highlightNode(node);
    });
    rowGroups.on("mouseleave", function (event, _) {
        if (event.toElement?.__data__) {
            const row = d3.select(this);
            row.attr("transform", null)
                .selectAll("rect")
                .attr("height", originalHeight);
            row.selectAll("text")
                .style("font-size", null) // Reset font size
                .attr("y", d => y(d.nodeId) + originalHeight / 2);
        }
    });

    return svg.node();
}


// Combines decoupled node and census response from API
function zipNodesAndCensusData(nodeIdsWithNickNames, censuses, enrs) {
    return nodeIdsWithNickNames.map(([nodeId, nickname], index) => {
        // Map enr_ids to their corresponding census status
        const censusInfos = censuses.map(census => {
            let censusInfo = census.nodes[index];
            if (censusInfo) {
                censusInfo.enr = enrs[censusInfo.nodeEnrId];
            }
            return censusInfo;
        });

        // Use the latest census info for display & sorting.
        const latestCensusInfo = censusInfos.findLast((censusInfo) => !!censusInfo);
        let clientString = null;
        let clientColor = null;
        if (latestCensusInfo) {
            clientString = `${latestCensusInfo.client.name} ${latestCensusInfo.clientVersion || ''}`;
            clientColor = latestCensusInfo.client.color;
        }

        return {
            nodeId: nodeId,
            nodeNickName: nickname,
            censusInfos: censusInfos,
            clientString: clientString,
            clientColor: clientColor,
        };
    });
}

// The same node will have the same ENR for multiple censuses.
// Cache the decoded ENR to avoid decoding the same ENR multiple times.
function decodeEnrCached(enr) {
    if (!decodedEnrs[enr]) {
        let { enr: decodedEnr, seq, _ } = ENR.ENR.decodeTxt(enr);
        let enrData = {
            ip: decodedEnr["ip"],
            port: decodedEnr["udp"],
            seq: seq,
            shortened: enr.substring(0, 15) + '...' + enr.substring(enr.length - 10)
        }
        decodedEnrs[enr] = enrData;
    }
    return decodedEnrs[enr];
}


// Helper function for creating hover-over text.
function createHoverOverInfo(censusInfo, time) {
    let decodedEnr = decodeEnrCached(censusInfo.enr);

    let title = `Census started at ${time}.\n`;
    title += `ENR: ${decodedEnr.shortened}\n`;
    title += `IP: ${decodedEnr.ip}:${decodedEnr.port}\n`
    title += `Seq: ${decodedEnr.seq}\n`
    title += `Radius: ${censusInfo.radiusAsPercentage}\n`;
    title += `Client: ${censusInfo.client.name}\n`;
    title += `Client Version: ${censusInfo.clientVersion || "Unknown"}\n`;
    title += `Client Short Commit: ${censusInfo.clientShortCommit || "Unknown"}\n`;

    return title;
}

// Fetch the census node records from the API.
async function getCensusTimeSeriesData(numDaysAgo, subprotocol) {

    const baseUrl = `census-node-timeseries-data/?days-ago=${numDaysAgo}&network=${subprotocol}`;
    return fetch(`${baseUrl}`)
        .then(response => {
            if (!response.ok) {
                throw new Error('Network response was not ok');
            }
            return response.json();
        })
        .catch(error => {
            console.error('There was a problem with the fetch operation:', error.message);
        });
}

// Create the census node timeseries chart using data from the API and add it to the DOM.
async function censusTimeSeriesChart(daysAgo) {
    document.querySelectorAll('svg').forEach(function (svgElement) {
        svgElement.remove();
    });

    const url = new URL(window.location);
    subprotocol = url.searchParams.get('network');

    const data = await getCensusTimeSeriesData(daysAgo, subprotocol);
    console.log('Census data from glados API:');
    console.log(data);
    if (data) {
        document.getElementById('census-timeseries-graph').appendChild(createSquareChart(1700, data));
    } else {
        console.log('No data available to plot the census chart');
    }
}

// Sort nodes by nickname, clientString, and nodeId
function sortNodes(a, b) {

    // Check for "bootnode" in nodeNickName
    const aIsBootnode = a.nodeNickName && a.nodeNickName.includes("bootnode");
    const bIsBootnode = b.nodeNickName && b.nodeNickName.includes("bootnode");

    // If one is a bootnode and the other isn't, prioritize the bootnode
    if (aIsBootnode && !bIsBootnode) return -1;
    if (!aIsBootnode && bIsBootnode) return 1;

    // If both nodes have nicknames, sort by nickname
    if (a.nodeNickName && b.nodeNickName) {
        // Extract the prefix and number from the nickname
        const [aPrefixA, aNumberA] = a.nodeNickName.split(/-(\d+)$/);
        const [bPrefixB, bNumberB] = b.nodeNickName.split(/-(\d+)$/);

        // If the prefixes are different, sort by the prefix
        if (aPrefixA !== bPrefixB) {
            return aPrefixA.localeCompare(bPrefixB);
        }

        // If the prefixes are the same, sort by the number
        return Number(aNumberA) - Number(bNumberB);
    }

    // If one has a nickname and the other doesn't, prioritize the one that has.
    if (a.nodeNickName && !b.nodeNickName) return -1;
    if (!a.nodeNickName && b.nodeNickName) return 1;

    // Place nodes with clientInfo after nodes with nicknames
    if (a.clientString && !b.clientString) return -1;
    if (!a.clientString && b.clientString) return 1;

    // If both nodes have clientString, sort by clientString
    if (a.clientString && b.clientString) {
        return a.clientString.localeCompare(b.clientString);
    }

    // Place nodes without nicknames or clientString at the end, sorted by nodeId
    return a.nodeId.localeCompare(b.nodeId);
}