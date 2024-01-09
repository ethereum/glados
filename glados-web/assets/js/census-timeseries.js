function createSquareChart(height, width, data) {

    // Incoming data takes the form:
   /*{
        "node_ids": [
          "0x9891fe3fdfcec2707f877306fc6e5596b1405fedf70ad2911496c2ac40dfeda5",
          "0x08653f55f8120591717aae0426c10c25f0e6a7db078c2e1516b4f3059cefff35",
          "0xbefb246b22757267608a436d265018374592291f284c536f73c933b29a91e10a"
        ],
        "censuses": [
          {
            "time": "2023-12-13T02:03:38.389580Z",
            "enr_statuses": [
              "enr:-Jy4QOOcmr_MEnF8HacDDUxXjCkaMDsvij3-lNBg...mlwhM69ZOyJc2VjcDI1NmsxoQNhyC_neIY2fOKxBi8UGBvnZiBcWbChOHq73nNxeu0ELYN1ZHCCIzE",
              null,
              null,
            ]
          },
          ...
        ]
      }*/

    // Declare the chart dimensions and margins.
    const marginTop = 80;
    const marginLeft = 40;

    // Create a combined array of node IDs and their statuses.
    const nodesAndEnrStatuses = zipNodesAndCensusData(data.node_ids, data.censuses);

    // Sort the combined array based on latestClientString and secondarily nodeId
    nodesAndEnrStatuses.sort((a, b) => {
        // Place empty at the end instead of front
        if (a.latestClientString === null && b.latestClientString !== null) return 1;
        if (a.latestClientString !== null && b.latestClientString === null) return -1;
        const clientComparison = (a.latestClientString || "").localeCompare(b.latestClientString || "");
        if (clientComparison !== 0) {
            return clientComparison;
        }
        // If latestClientString is equal, then compare by nodeId
        return a.nodeId.localeCompare(b.nodeId);
    });

    const x = d3.scaleTime()
        .domain(d3.extent(data.censuses, d => new Date(d.time)))
        .range([0, width]);

    const nodes = nodesAndEnrStatuses.map(d => d.nodeId);
    const y = d3.scaleBand()
        .domain(nodes)
        .range([0, height])
        .padding(0.05);  

    // Parameters for handling scaling of rows.
    const originalHeight = y.bandwidth();
    const expandedScaleFactor = 12.0;
    const expandedCellHeight = (originalHeight * expandedScaleFactor)
        
    // Create the SVG container.
    const svg = d3.create("svg")
        .attr("width", width)
        .attr("height", height + expandedCellHeight + marginTop)
        .attr("viewBox", [0, 0, width, height + expandedCellHeight + marginTop])
        .attr("overflow", "visible")
        .attr("style", "max-width: 100%; height: auto; height: intrinsic;");

    // Append the title
    svg.append("text")
       .attr("x", width / 2)
       .attr("y", 10)
       .attr("text-anchor", "middle")
       .style("font-size", "24px")
       .text("24 Hour Census Results");

    // Append the subtitle
    svg.append("text")
        .attr("x", width / 2)
        .attr("y", 40)
        .attr("text-anchor", "middle")
        .style("font-size", "16px")
        .text(`24 hour period beginning ${data.censuses[0].time}`);
    
    // Append X axis to the bottom and top
    svg.append("g")
        .attr("transform", `translate(0,${marginTop + height})`)
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
        .data(nodesAndEnrStatuses)
        .enter()
        .append("g")
        .attr("class", "row-group")
        .attr("id", (d, i) => `row-${i}`);

    rowGroups.attr("transform", `translate(0, ${marginTop})`);

    // Append squares to each row group.
    rowGroups.each(function(node, i) {
        node.statuses.forEach((censusResult, j) => {
            const status = censusResult;

            const row = d3.select(this);
            const rect = row.append("rect")
              .attr("x", x(new Date(data.censuses[j].time)))
              .attr("y", y(node.nodeId))
              .attr("width", 17)
              .attr("height", y.bandwidth())
              .attr("fill", status ? "green" : "gray");

            let title = "";
            if (censusResult) {
                title = createHoverOverInfoFromENR(censusResult, data.censuses[j].time);
            } else {
                title = `Not found during the census beginning at ${data.censuses[j].time}.`;
            }

            rect.append("title").text(title);

            rect.on("click", function(event, d) {
                window.open(`/census/?census-id=${data.censuses[j].census_id}`, '_blank');
            });
            
            rect.on("mouseover", function(event) {
                row.raise();
                rect.raise()
                     .style("stroke", "white")
                     .style("stroke-width", 4);
            }).on("mouseout", function() {
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
        .text(d => (d.nodeId.substring(0, 6) + '...' + d.nodeId.substring(d.nodeId.length - 4)));

    // Internal function to handle hover-over magnification effect.
    function highlightNode(node) {
        let hoveredIndex = nodes.indexOf(node.nodeId);
        let downwardShift = 0;

        // Push x axis down to fit expanded rows
        svg.select("g.x-axis")
            .attr("transform", `translate(0,${height + expandedCellHeight + marginTop - originalHeight})`);
        svg.attr("height", height + expandedCellHeight + marginTop);

         // Reset all rows to their original positions and heights, if necessary
         rowGroups.attr("transform", null)
            .selectAll("rect")
            .attr("height", originalHeight);

         // Reset font size and position of text, if necessary
         rowGroups.selectAll("text")
             .style("font-size", null) // Reset font size
             .attr("y", d => y(d.nodeId) + originalHeight / 2);

        // Remove existing client string labels if any.
        svg.selectAll(".client-string-label").remove();

        rowGroups.each(function(_, index) {
            const group = d3.select(this);
            let scaleFactor, heightIncrease;
    
            if (index === hoveredIndex) {
                scaleFactor = expandedScaleFactor;
                // Adjust the height of the row
                group.selectAll("rect")
                    .attr("height", expandedCellHeight);
            } else {
                scaleFactor = 1;
            }
    
            heightIncrease = (originalHeight * scaleFactor) - originalHeight;
            downwardShift += heightIncrease;
    
            // Adjust the font size and position of the text
            group.selectAll("text")
                .style("font-size", `${1 + (0.7 * ((scaleFactor - 1) / (expandedScaleFactor - 1)))}em`)
                .attr("y", y(nodesAndEnrStatuses[index].nodeId) + (originalHeight * scaleFactor) / 2);
    
            // Shift rows below the hovered row downwards
            group.attr("transform", `translate(0, ${marginTop + (downwardShift - heightIncrease)})`);

            // Append metadata
            if (index === hoveredIndex) {
                const yPos = y(nodesAndEnrStatuses[index].nodeId) + (originalHeight * scaleFactor) / 2;

                // Append a new text element for latestClientString
                group.append("text")
                    .attr("class", "client-string-label")
                    .attr("x", -200)
                    .attr("y", yPos + 30) // Adjust this to position the label correctly
                    .attr("text-anchor", "start")
                    .text(nodesAndEnrStatuses[index].latestClientString);
            }

        });
    }

    // Apply hover effect to the row groups
    rowGroups.on("mouseover", function(_, node) {
        highlightNode(node);
    });

    return svg.node();
}

// Combines decoupled node and census response from API
function zipNodesAndCensusData(nodes, censuses) {
    return nodes.map((nodeId, index) => {

        // Use the latest ENR for display & sorting.
        let enrString = censuses[censuses.length - 1].enr_statuses[index];
        let clientName = null;
        if (enrString) {
            let {enr: decodedEnr, seq, signature} = ENR.ENR.decodeTxt(enrString);
            
            for (let [key, value] of decodedEnr.entries()) {
                if (key === "c") {
                    let fullClientString = String.fromCharCode.apply(null, value);
                    if (fullClientString[0] === 'f') {
                        clientName = "fluffy ";
                    } 
                    else if (fullClientString[0] === 't') {
                        clientName = "trin ";
                        clientName += fullClientString.substring(2);
                    } else {
                        clientName = fullClientString;
                    }

                }
            }
        }
        return {
            nodeId: nodeId,
            latestClientString: clientName,
            statuses: censuses.map(census => census.enr_statuses[index])
        };
    });
}

// Helper function for creating hover-over text.
function createHoverOverInfoFromENR(enr, time) {

    let {enr: decodedEnr, seq, signature} = ENR.ENR.decodeTxt(enr);
    let ip = decodedEnr["ip"];
    let port = decodedEnr["udp"];
    let shortenedEnr = enr.substring(0, 15) + '...' + enr.substring(enr.length - 10);
    let clientString = getClientStringFromDecodedEnr(decodedEnr);
    title = `Node found during census beginning at ${time}.\nENR: ${shortenedEnr}\nIP: ${ip}:${port}\nSeq: ${seq}\nClient String: ${clientString}`;

    return title;

}

function getClientStringFromDecodedEnr(decodedEnr) {
    for (let [key, value] of decodedEnr.entries()) {

        if (key === "c") {
            return String.fromCharCode.apply(null, value);
        } else {
            return "";
        }
    }

}

// Fetch the census node records from the API.
async function getCensusTimeSeriesData(daysAgo) {

    const baseUrl = `census-node-timeseries-data/?days-ago=daysAgo`;

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
async function censusTimeSeriesChart() {
        const data = await getCensusTimeSeriesData();

        if (data) {
            document.getElementById('census-timeseries-graph').appendChild(createSquareChart(1700, 1700, data));
        } else {
            console.log('No data available to plot the census chart');
        }
}
