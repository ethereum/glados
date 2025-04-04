let daysAgo = 0;
let decodedEnrs = {};
let highlightedRowGroup = null;

function createSquareChart(width, data) {
  // Incoming data takes the following form.
  // node_ids and enr_statuses are arrays of the same length and order.
  /*{
         "node_ids": [
           ["0x9891fe3fdfcec2707f877306fc6e5596b1405fedf70ad2911496c2ac40dfeda5", null],
           ["0x08653f55f8120591717aae0426c10c25f0e6a7db078c2e1516b4f3059cefff35", "node-nickname"],
           ["0xbefb246b22757267608a436d265018374592291f284c536f73c933b29a91e10a", null]
         ],
         "censuses": [
           {
             "time": "2023-12-13T02:03:38.389580Z",
             "enr_statuses": [
               345,
               null,
               null,
             ]
           },
           ...
         ],
         "enrs" {
            345: "enr:-Jy4QOOcmr_MEnFj3-lNBg...ZiBcWbChOHq73nNxeu0ELYN1ZHCCIzE"
        }
       }*/

  // Create a combined array of node IDs and their ENRs during each census.
  let nodesAndEnrStatuses = zipNodesAndCensusData(
    data.node_ids_with_nicknames,
    data.censuses,
    data.enrs,
  );

  // Sort the combined array based on latestClientString and secondarily nodeId
  nodesAndEnrStatuses.sort((a, b) => sortNodes(a, b));

  console.log(`Number of nodes: ${nodesAndEnrStatuses.length}`);

  const x = d3
    .scaleTime()
    .domain(d3.extent(data.censuses, (d) => new Date(d.time)))
    .range([0, width]);

  const nodes = nodesAndEnrStatuses.map((d) => d.nodeId);

  const cellHeight = 11;
  const height = cellHeight * nodes.length;
  const y = d3.scaleBand().domain(nodes).range([0, height]).padding(0.05);

  // Parameters for handling scaling of rows.
  const originalHeight = y.bandwidth();
  const expandedScaleFactor = 12.0;
  const expandedCellHeight = originalHeight * expandedScaleFactor;

  // Create the SVG container.
  const marginTop = 110;
  const marginLeft = 30;
  const svg = d3
    .create("svg")
    .attr("width", width)
    .attr("height", height + expandedCellHeight + marginTop)
    .attr("viewBox", [
      -(width * 0.07),
      0,
      width,
      height + expandedCellHeight + marginTop,
    ])
    .attr("overflow", "visible")
    .attr("style", "max-width: 90%; height: 100%; height: intrinsic;");

  const url = new URL(window.location);
  const subprotocol = url.searchParams.get("network");
  let title =
    data.censuses.length > 0
      ? `${nodes.length} ${subprotocol} nodes found during 24 hour period beginning at ${data.censuses[0].time}`
      : `No ${subprotocol} censuses found during this 24 hour period.`;

  // Append the title
  svg
    .append("text")
    .attr("x", width / 2)
    .attr("y", 30)
    .attr("text-anchor", "middle")
    .style("font-size", "24px")
    .text(title);

  // Append the previous button
  svg
    .append("a")
    .attr("xlink:href", "#")
    .on("click", function (event) {
      event.preventDefault();
      console.log(`days ago: ${daysAgo}`);
      censusTimeSeriesChart(daysAgo + 1);
      daysAgo++;
      console.log(`days ago now: ${daysAgo}`);
    })
    .append("text")
    .attr("x", width / 2 - 130)
    .attr("y", 70)
    .text("< Previous");

  // Append the next button
  svg
    .append("a")
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
    })
    .append("text")
    .attr("x", width / 2 + 70)
    .attr("y", 70)
    .text("Next >");

  // Append X axis to the bottom and top
  svg
    .append("g")
    .attr(
      "transform",
      `translate(0,${height + expandedCellHeight + marginTop - originalHeight})`,
    )
    .attr("class", "x-axis")
    .call(d3.axisBottom(x))
    .selectAll("text")
    .style("font-size", "12px");
  svg
    .append("g")
    .attr("transform", `translate(0,${marginTop})`)
    .attr("class", "x-axis")
    .call(d3.axisTop(x))
    .selectAll("text")
    .style("font-size", "12px");

  // Create group for each row.
  const rowGroups = svg
    .selectAll(".row-group")
    .data(nodesAndEnrStatuses)
    .enter()
    .append("g")
    .attr("class", "row-group")
    .attr("id", (d, i) => `row-${i}`);

  rowGroups.attr("transform", `translate(0, ${marginTop})`);

  data.censuses.forEach((census) => {
    census.parsedTime = new Date(census.time);
  });

  // Append squares to each row group.
  rowGroups.each(function (node, i) {
    node.statuses.forEach((censusResult, j) => {
      const row = d3.select(this);
      const rect = row
        .append("rect")
        .attr("x", x(data.censuses[j].parsedTime))
        .attr("y", y(node.nodeId))
        .attr("width", `${(width * 0.96) / data.censuses.length}px`)
        .attr("height", y.bandwidth() + "px")
        .attr("stroke-width", 0.1)
        .attr("stroke", "rgb(245, 245, 245)")
        .attr("fill", censusResult ? "green" : "gray");

      let title = "";
      if (censusResult) {
        title = createHoverOverInfoFromENR(censusResult, data.censuses[j].time);
      } else {
        title = `Not found during the census beginning at ${data.censuses[j].time}.`;
      }

      rect.append("title").text(title);

      rect.on("click", function (_, _) {
        window.open(
          `/census/?census-id=${data.censuses[j].census_id}`,
          "_blank",
        );
      });

      rect
        .on("mouseenter", function (_) {
          rect.raise().style("stroke", "white").style("stroke-width", 4);
        })
        .on("mouseout", function () {
          d3.select(this).style("stroke", null).style("stroke-width", null);
        });
    });
  });

  // Append y-axis labels/link to each row group
  rowGroups
    .append("a")
    .attr("xlink:href", (d) => `/network/node/${d.nodeId}/`)
    .attr("target", "_blank")
    .append("text")
    .attr("x", -marginLeft)
    .attr("y", (d) => y(d.nodeId) + y.bandwidth() / 2)
    .attr("dy", ".35em")
    .attr("text-anchor", "end")
    .text(
      (d) =>
        d.nodeId.substring(0, 6) +
        "..." +
        d.nodeId.substring(d.nodeId.length - 4),
    );

  // Internal function to handle hover-over magnification effect.
  function highlightNode(node) {
    let hoveredIndex = nodes.indexOf(node.nodeId);

    let downwardShift = 0;

    // Remove existing client string labels if any.
    svg.selectAll(".client-string-label").remove();
    // Unhighlight the highlighted row.
    if (highlightedRowGroup) {
      highlightedRowGroup
        .attr("transform", null)
        .selectAll("rect")
        .attr("height", originalHeight);
      highlightedRowGroup
        .selectAll("text")
        .style("font-size", null) // Reset font size
        .attr("y", (d) => y(d.nodeId) + originalHeight / 2);
    }

    rowGroups.each(function (_, index) {
      const group = d3.select(this);
      let scaleFactor, heightIncrease;

      if (index === hoveredIndex) {
        scaleFactor = expandedScaleFactor;
        // Adjust the height of the row
        group.selectAll("rect").attr("height", expandedCellHeight);
        group.selectAll("text").style("font-size", "1.7em");
      } else {
        scaleFactor = 1;
      }

      heightIncrease = originalHeight * scaleFactor - originalHeight;
      downwardShift += heightIncrease;

      // Adjust the font size and position of the text
      group
        .selectAll("text")
        .attr(
          "y",
          y(nodesAndEnrStatuses[index].nodeId) +
            (originalHeight * scaleFactor) / 2,
        );

      // Shift rows below the hovered row downwards
      group.attr(
        "transform",
        `translate(0, ${marginTop + (downwardShift - heightIncrease)})`,
      );

      // Append metadata
      if (index === hoveredIndex) {
        const yPos =
          y(nodesAndEnrStatuses[index].nodeId) +
          (originalHeight * scaleFactor) / 2;

        // Append a new text element for latestClientString
        group
          .append("text")
          .attr("class", "client-string-label")
          .attr("x", -200)
          .attr("y", yPos + 30)
          .attr("text-anchor", "start")
          .text(nodesAndEnrStatuses[index].latestClientString);
        group
          .append("text")
          .attr("class", "client-string-label")
          .attr("x", -200)
          .attr("y", yPos + -20)
          .attr("text-anchor", "start")
          .text(nodesAndEnrStatuses[index].nodeNickName);
      }
    });

    highlightedRowGroup = rowGroups.filter((d) => d.nodeId === node.nodeId);
  }

  // Apply hover effect to the row groups
  rowGroups.on("mouseenter", function (_, node) {
    highlightNode(node);
  });
  rowGroups.on("mouseleave", function (event, _) {
    if (event.toElement?.__data__) {
      const row = d3.select(this);
      row
        .attr("transform", null)
        .selectAll("rect")
        .attr("height", originalHeight);
      row
        .selectAll("text")
        .style("font-size", null) // Reset font size
        .attr("y", (d) => y(d.nodeId) + originalHeight / 2);
    }
  });

  return svg.node();
}

// Combines decoupled node and census response from API
function zipNodesAndCensusData(nodeIdsWithNickNames, censuses, records) {
  return nodeIdsWithNickNames.map(([nodeId, nickname], index) => {
    // Map enr_ids to their corresponding recordString
    const statuses = censuses.map((census) => {
      const enrId = census.enr_statuses[index];
      return records[enrId] || null;
    });

    // Use the latest ENR for display & sorting. Iterate backwards if the node's isn't seen in the latest.
    let enrString = null;
    for (let i = statuses.length - 1; i > 0; i--) {
      enrString = statuses[i];
      if (enrString) {
        break;
      }
    }

    // Get the client name for sorting and display purposes.
    let clientName = null;
    if (enrString) {
      let decodedEnr = decodeEnrCached(enrString);
      let fullClientString = decodedEnr.client;
      if (fullClientString !== null) {
        if (fullClientString[0] === "f") {
          clientName = "fluffy ";
        } else if (fullClientString[0] === "t") {
          clientName = "trin ";
          clientName += fullClientString.substring(2);
        } else if (fullClientString[0] === "u") {
          clientName = "ultralight";
        } else {
          clientName = fullClientString;
        }
      }
    }

    return {
      nodeId: nodeId,
      nodeNickName: nickname,
      latestClientString: clientName,
      statuses: statuses,
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
      client: getClientStringFromDecodedEnr(decodedEnr),
      shortened: enr.substring(0, 15) + "..." + enr.substring(enr.length - 10),
    };
    decodedEnrs[enr] = enrData;
  }
  return decodedEnrs[enr];
}

// Helper function for creating hover-over text.
function createHoverOverInfoFromENR(enr, time) {
  let decodedEnr = decodeEnrCached(enr);
  let title = `Node found during census beginning at ${time}.\nENR: ${decodedEnr.shortened}\nIP: ${decodedEnr.ip}:${decodedEnr.port}\nSeq: ${decodedEnr.seq}\nClient String: ${decodedEnr.client}`;

  return title;
}

function getClientStringFromDecodedEnr(decodedEnr) {
  for (let [key, value] of decodedEnr.entries()) {
    if (key === "c") {
      return String.fromCharCode.apply(null, value);
    } else {
      return null;
    }
  }
}

// Fetch the census node records from the API.
async function getCensusTimeSeriesData(numDaysAgo, subprotocol) {
  const baseUrl = `census-node-timeseries-data/?days-ago=${numDaysAgo}&network=${subprotocol}`;
  return fetch(`${baseUrl}`)
    .then((response) => {
      if (!response.ok) {
        throw new Error("Network response was not ok");
      }
      return response.json();
    })
    .catch((error) => {
      console.error(
        "There was a problem with the fetch operation:",
        error.message,
      );
    });
}

// Create the census node timeseries chart using data from the API and add it to the DOM.
async function censusTimeSeriesChart(daysAgo) {
  document.querySelectorAll("svg").forEach(function (svgElement) {
    svgElement.remove();
  });

  const url = new URL(window.location);
  subprotocol = url.searchParams.get("network");

  const data = await getCensusTimeSeriesData(daysAgo, subprotocol);
  console.log("Census data from glados API:");
  console.log(data);
  if (data) {
    document
      .getElementById("census-timeseries-graph")
      .appendChild(createSquareChart(1700, data));
  } else {
    console.log("No data available to plot the census chart");
  }
}

// Sort nodes by nickname, latestClientString, and nodeId
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

  if (a.nodeNickName && !b.nodeNickName) return -1;
  if (!a.nodeNickName && b.nodeNickName) return 1;

  // Place nodes with latestClientString after nodes with nicknames
  if (a.latestClientString && !b.latestClientString) return -1;
  if (!a.latestClientString && b.latestClientString) return 1;

  // If both nodes have latestClientString, sort by latestClientString
  if (a.latestClientString && b.latestClientString) {
    return a.latestClientString.localeCompare(b.latestClientString);
  }

  // Place nodes without nicknames or latestClientString at the end, sorted by nodeId
  return a.nodeId.localeCompare(b.nodeId);
}
