function createMultiLineChart(
  height,
  width,
  dataSets,
  weeksAgo,
  labels = null,
  selectedIndexes = null,
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

  // Color palette for the lines.
  const colors = d3.schemeTableau10;

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

  // Add Y axis
  // Declare the y (vertical position) scale.
  const y = d3
    .scaleLinear()
    .domain([0, d3.max(dataSets.flat(), (d) => d.value)])
    .range([height - marginBottom, marginTop]);

  svg
    .append("g")
    .attr("transform", `translate(${marginLeft} ,0)`)
    .call(d3.axisLeft(y))
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
        .text("# of nodes"),
    );

  // Add title
  svg
    .append("text")
    .attr("class", "graph-title")
    .attr("text-anchor", "middle")
    .attr("x", width / 2)
    .attr("y", marginTop / 2)
    .text("Nodes in Census, by week");

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

function convertDataForChart(data) {
  return data.map((d) => ({
    date: new Date(d.start),
    value: d.nodeCount,
  }));
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

function getCurrentSubprotocolName() {
  const url = new URL(window.location);
  const subprotocolName = url.searchParams.get("network")
    ? url.searchParams.get("network").toLowerCase()
    : "history";

  return subprotocolName;
}

async function updateChart(weeksAgo) {
  const subprotocolName = getCurrentSubprotocolName();
  const BASE_URL = "/api/census-weekly/?";

  let params = new URLSearchParams();
  params.append("network", subprotocolName);
  params.append("weeks-ago", weeksAgo);

  const data = await getStatsRecords(BASE_URL + params);

  let dataSets = [convertDataForChart(data)];

  // Clear the existing chart
  d3.select("#census-weekly-graph").html("");

  // Create a new chart with the updated data
  if (dataSets && dataSets.length > 0) {
    document
      .getElementById("census-weekly-graph")
      .appendChild(createMultiLineChart(425, 1060, dataSets, weeksAgo));
  } else {
    console.log("No data available to plot the stats chart");
  }
}

export var censusWeeklyChart = async function () {
  updateChart(0);
};
