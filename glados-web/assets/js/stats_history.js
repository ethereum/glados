function createMultiLineChart(
  height,
  width,
  dataSets,
  labels,
  selectedIndexes,
  weeksAgo,
) {
  // Declare the chart dimensions and margins.
  const marginTop = 40;
  const marginRight = 50;
  const marginBottom = 20;
  const marginLeft = 40;

  // Parse dates if they're not already Date objects
  dataSets.forEach((dataset) => {
    dataset.forEach((d) => {
      if (!(d.date instanceof Date)) d.date = new Date(d.date);
    });
  });

  // Declare the x (horizontal position) scale.
  const x = d3
    .scaleTime()
    .domain(d3.extent(dataSets.flat(), (d) => d.date))
    .range([marginLeft, width - marginRight]);

  // Declare the y (vertical position) scale.
  const y = d3
    .scaleLinear()
    .domain([0, d3.max(dataSets.flat(), (d) => d.value)])
    .range([height - marginBottom, marginTop]);

  // Color palette for the lines.
  const colors = d3.schemeTableau10;

  // Array to keep track of which datasets are visible.
  let visibility = new Array(dataSets.length).fill(true);

  // Create the SVG container.
  const svg = d3
    .create("svg")
    .attr("width", width)
    .attr("height", height)
    .attr("viewBox", [0, 0, width, height])
    .attr("overflow", "visible")
    .attr("style", "max-width: 100%; height: auto; height: intrinsic;");

  // Add the x-axis.
  svg
    .append("g")
    .attr("transform", `translate(0,${height - marginBottom})`)
    .call(
      d3
        .axisBottom(x)
        .ticks(width / 80)
        .tickSizeOuter(0),
    );

  // Add the y-axis, remove the domain line, add grid lines and a label.
  svg
    .append("g")
    .attr("transform", `translate(${marginLeft},0)`)
    .call(
      d3
        .axisLeft(y)
        .ticks(height / 40)
        .tickFormat((d) => d + "%"),
    )
    .call((g) => g.select(".domain").remove())
    .call((g) =>
      g
        .selectAll(".tick line")
        .clone()
        .attr("x2", width - marginLeft - marginRight)
        .attr("stroke-opacity", 0.1),
    )
    .call((g) =>
      g
        .append("text")
        .attr("x", -marginLeft)
        .attr("y", 10)
        .attr("fill", "currentColor")
        .attr("text-anchor", "start")
        .text("↑ Success Rate"),
    );

  // Add title
  svg
    .append("text")
    .attr("class", "graph-title")
    .attr("text-anchor", "middle")
    .attr("x", width / 2)
    .attr("y", marginTop / 2)
    .text("Audit Success, by week");

  // Add lines to the graph.
  const lines = dataSets.map((dataSet, i) => {
    const line = d3
      .line()
      .defined((d) => !isNaN(d.value))
      .x((d) => x(d.date))
      .y((d) => y(d.value));

    return svg
      .append("path")
      .datum(dataSet)
      .attr("fill", "none")
      .attr("stroke", colors[i % colors.length])
      .attr("stroke-width", 1.5)
      .attr("d", line)
      .attr("class", `line line-${i}`);
  });

  // Add a legend.
  const legend = svg
    .selectAll(".legend")
    .data(dataSets)
    .enter()
    .append("g")
    .attr("class", "legend")
    .attr(
      "transform",
      (d, i) => `translate(${width - marginRight + 100}, ${i * 20 + 30})`,
    ) // Position adjusted to the right
    .style("font", "10px sans-serif")
    .style("cursor", "pointer")
    .on("click", function (event, d) {
      const index = dataSets.indexOf(d);
      visibility[index] = !visibility[index];
      d3.select(lines[index].node()).style(
        "opacity",
        visibility[index] ? 1 : 0,
      );
      d3.select(this).style("opacity", visibility[index] ? 1 : 0.5); // Adjust the legend item's opacity
    });

  // Function to dispatch a click event
  function dispatchClick(element) {
    const clickEvent = new MouseEvent("click", {
      view: window,
      bubbles: true,
      cancelable: true,
    });
    element.dispatchEvent(clickEvent);
  }

  // Select all '.legend' group elements and click on all but the first three
  svg.selectAll(".legend").each(function (d, i) {
    if (!selectedIndexes.includes(i)) {
      dispatchClick(this);
    }
  });

  // Add colored rectangles to the legend.
  legend
    .append("rect")
    .attr("x", -18)
    .attr("width", 18)
    .attr("height", 18)
    .attr("fill", (d, i) => colors[i % colors.length]);

  // Add text to the legend.
  legend
    .append("text")
    .attr("x", -24)
    .attr("y", 9)
    .attr("dy", ".35em")
    .attr("text-anchor", "end")
    .text((d, i) => labels[i])
    .attr("class", "legend-text");

  // Append a vertical line to the chart, initially hidden
  const verticalLine = svg
    .append("line")
    .style("stroke", "grey")
    .style("stroke-width", "1px")
    .style("stroke-dasharray", "3,3")
    .style("display", "none"); // Initially hidden

  // Create an overlay for the mouseover event
  svg
    .append("rect")
    .attr("class", "overlay")
    .attr("x", marginLeft)
    .attr("y", marginTop)
    .attr("width", width - marginLeft - marginRight)
    .attr("height", height - marginTop - marginBottom)
    .style("fill", "none")
    .style("pointer-events", "all")
    .on("mouseover", () => tooltip.style("display", null))
    .on("mouseout", () => {
      verticalLine.style("display", "none");
      tooltip.style("display", "none");
    })
    .on("mousemove", mousemove);

  // Function to handle mouse movements
  function mousemove(event) {
    const x0 = x.invert(d3.pointer(event)[0]),
      formatDate = d3.timeFormat("%H:%M:%S");
    const xPos = d3.pointer(event)[0];
    tooltip
      .attr("transform", `translate(${d3.pointer(event)[0]},0)`)
      .call((g) => g.select("text").text(`${formatDate(x0)}`));

    // Update the vertical line position
    verticalLine
      .attr("x1", xPos)
      .attr("x2", xPos)
      .attr("y1", marginTop)
      .attr("y2", height - marginBottom)
      .style("display", null);
  }

  // Create a tooltip for displaying the time
  const tooltip = svg.append("g").style("display", "none");

  tooltip
    .append("text")
    .attr("class", "tooltip-date")
    .attr("y", marginTop - 4)
    .attr("x", marginLeft)
    .attr("text-anchor", "middle")
    .attr("font-size", "12px")
    .style("background", "white");

  // Add buttons to the bottom of the legend
  const buttons = svg
    .append("g")
    .attr(
      "transform",
      `translate(${width - marginRight + 20}, ${dataSets.length * 20 + 30})`,
    );

  // Add the previous button ("<")
  buttons
    .append("text")
    .attr("class", "previous-button")
    .attr("x", 0)
    .attr("y", 30)
    .attr("font-size", "12px")
    .attr("text-anchor", "start")
    .text("< Previous")
    .style("cursor", "pointer")
    .on("click", () => {
      weeksAgo++;
      updateChart(weeksAgo);
    });

  if (weeksAgo !== 0) {
    // Add the next button (">")
    buttons
      .append("text")
      .attr("class", "next-button")
      .attr("x", 15)
      .attr("y", 50)
      .attr("font-size", "12px")
      .attr("text-anchor", "start")
      .text("Next >")
      .style("cursor", "pointer")
      .on("click", () => {
        if (weeksAgo > 0) {
          weeksAgo--;
          updateChart(weeksAgo);
        }
      });
  }

  return svg.node();
}

function convertDataForChart(data, keys) {
  return keys.map((key) =>
    data.map((d) => ({
      date: new Date(d.timestamp),
      value: d[key],
    })),
  );
}

function getStatsRecords(statsUrl) {
  return fetch(statsUrl)
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

function getCurrentSubprotocol() {
  const url = new URL(window.location);
  const subprotocolName = url.searchParams.get("network")
    ? url.searchParams.get("network").toLowerCase()
    : "history";

  subprotocols = {
    history: {
      baseUrl: "api/stats-history/?weeks-ago=",
      keys: [
        "success_rate_history_all",
        "success_rate_history_latest",
        "success_rate_history_random",
        "success_rate_history_four_fours",
        "success_rate_history_all_headers",
        "success_rate_history_all_headers_by_number",
        "success_rate_history_all_bodies",
        "success_rate_history_all_receipts",
        "success_rate_history_latest_headers",
        "success_rate_history_latest_headers_by_number",
        "success_rate_history_latest_bodies",
        "success_rate_history_latest_receipts",
        "success_rate_history_random_headers",
        "success_rate_history_random_headers_by_number",
        "success_rate_history_random_bodies",
        "success_rate_history_random_receipts",
        "success_rate_history_four_fours_headers",
        "success_rate_history_four_fours_headers_by_number",
        "success_rate_history_four_fours_bodies",
        "success_rate_history_four_fours_receipts",
      ],
      selectedIndexes: [2, 3],
      labels: [
        "All",
        "Latest",
        "Random",
        "4444s",
        "All Headers",
        "All Headers by #",
        "All Bodies",
        "All Receipts",
        "Latest Headers",
        "Latest Headers by #",
        "Latest Bodies",
        "Latest Receipts",
        "Random Headers",
        "Random Headers by #",
        "Random Bodies",
        "Random Receipts",
        "4444s Headers",
        "4444s Headers by #",
        "4444s Bodies",
        "4444s Receipts",
      ],
    },
  };

  return subprotocols[subprotocolName];
}

async function updateChart(weeksAgo) {
  const subprotocol = getCurrentSubprotocol();
  const data = await getStatsRecords(subprotocol.baseUrl + weeksAgo);

  let dataSets = convertDataForChart(data, subprotocol.keys);

  // Clear the existing chart
  d3.select("#stats-history-graph").html("");

  // Create a new chart with the updated data
  if (dataSets && dataSets.length > 0) {
    document
      .getElementById("stats-history-graph")
      .appendChild(
        createMultiLineChart(
          460,
          670,
          dataSets,
          subprotocol.labels,
          subprotocol.selectedIndexes,
          weeksAgo,
        ),
      );
  } else {
    console.log("No data available to plot the stats chart");
  }
}

async function statsHistoryChart() {
  updateChart(0);
}
