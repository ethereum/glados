import { Spinner } from "./spin.js";

const BASE_URL = "/api/audit-block-status/";
const GENESIS_TIMESTAMP = "2015-07-30T15:26:13Z";
const LAST_MINED_BLOCK_TIMESTAMP = "2022-09-15T06:42:42Z";

var spinerOpts = {
  lines: 20, // The number of lines to draw
  length: 20, // The length of each line
  width: 50, // The line thickness
  radius: 35, // The radius of the inner circle
  scale: 1, // Scales overall size of the spinner
  corners: 1, // Corner roundness (0..1)
  speed: 1, // Rounds per second
  rotate: 0, // The rotation offset
  animation: "spinner-line-fade-quick", // The CSS animation name for the lines
  direction: 1, // 1: clockwise, -1: counterclockwise
  color: ["#1275ed"], // CSS color or array of colors
  fadeColor: ["transparent"], // CSS color or array of colors
  top: "50%", // Top position relative to parent
  left: "50%", // Left position relative to parent
  shadow: "0 0 1px transparent", // Box-shadow for the lines
  zIndex: 2000000000, // The z-index (defaults to 2e9)
  className: "spinner", // The CSS class to assign to the spinner
  position: "absolute", // Element positioning
};

const target = document.getElementById("audit-block-status-graph-container");

const formatDate = d3.utcFormat("%Y-%m-%d %H:%M");
const formatValue = d3.format(",");
const formatPercentage = d3.format(".0%");
const tp = d3.utcParse("%Y-%m-%dT%H:%M:%S%Z");

function parseData(rawData, keys) {
  return rawData.map((period) => {
    const sum = keys.reduce(
      (sum, currentKey) => (sum += period[currentKey]),
      0,
    );

    const percentages = keys.reduce((ret, currentKey) => {
      ret[currentKey] = sum == 0 ? 0 : period[currentKey] / sum;
      return ret;
    }, {});

    return {
      ...period,
      date: tp(period.start),
      percentages,
    };
  });
}

async function loadChart() {
  // Declare the chart dimensions and margins.
  const width = 1060;
  const height = 425;
  const marginTop = 40;
  const marginRight = 50;
  const marginBottom = 20;
  const marginLeft = 40;

  const seriesProps = [
    {
      label: "success",
      stroke: "green",
      fill: "lightgreen",
    },
    {
      label: "error",
      stroke: "#e41a1c",
      fill: "pink",
    },
    {
      label: "unaudited",
      stroke: "#aaaaaa",
      fill: "#dddddd",
    },
  ];

  let data = [];

  const keys = seriesProps.map((series) => series.label);

  // Declare the x (horizontal position) scale.
  const x = d3.scaleUtc(
    [tp(GENESIS_TIMESTAMP), tp(LAST_MINED_BLOCK_TIMESTAMP)],
    [marginLeft, width - marginRight],
  );

  // Declare the y (vertical position) scale.
  const y = d3.scaleLinear([0, 1], [height - marginBottom, marginTop]);

  // Configure the stack function to stack the series
  const stacker = d3
    .stack()
    .keys(keys)
    .value((d, key) => d.percentages[key]);

  const areaGenerator = d3
    .area()
    .x((d) => x(d.data.date))
    .y0((d) => y(d[0]))
    .y1((d) => y(d[1]));

  const lineGenerator = d3
    .line()
    .x((d) => x(d.data.date))
    .y((d) => y(d[1]));

  // Create the SVG container.
  const svg = d3
    .select("#audit-block-status-graph")
    .append("svg")
    .attr("width", width)
    .attr("height", height)
    .attr("viewBox", [0, 0, width, height])
    .attr("id", "audit-block-status-graph-svg")
    .attr("style", "max-width: 100%; height: auto;");

  const dataGroup = svg.append("g");

  const focus = svg.append("g").style("visibility", "hidden");

  const brush = d3
    .brushX()
    .extent([
      [marginLeft, marginTop],
      [width - marginRight, height - marginBottom],
    ])
    .on("end", zoomChart)
    .on("brush", brushing);

  let series = seriesProps
    .map((series) => {
      const serieArea = dataGroup
        .append("path")
        .datum(series.data)
        .attr("class", series.label)
        .style("fill", series.color);

      return {
        ...series,
        area: serieArea,
      };
    })
    .map((series) => {
      const serieLine = dataGroup
        .append("path")
        .datum(series.data)
        .attr("fill", "none")
        .attr("stroke", "none")
        .attr("stroke-width", 2);

      return {
        ...series,
        line: serieLine,
      };
    });

  // Add the x-axis.
  const xAxis = svg
    .append("g")
    .attr("transform", `translate(0,${height - marginBottom})`)
    .call(d3.axisBottom(x));

  // Add the y-axis, remove the domain line, add grid lines and a label.
  const yAxis = svg
    .append("g")
    .attr("transform", `translate(${marginLeft},0)`)
    .call(
      d3
        .axisLeft(y)
        .ticks(height / 40)
        .tickFormat(formatPercentage),
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
        .text("% of blocks"),
    );

  const context = svg
    .append("g")
    .on("mouseover", function () {
      focus.style("visibility", null);
      tooltip.style("visibility", null);
    })
    .on("mouseout", function () {
      focus.style("visibility", "hidden");
      tooltip.style("visibility", "hidden");
    })
    .on("mousemove", mousemove)
    .call(brush);

  // Add title
  svg
    .append("text")
    .attr("class", "graph-title")
    .attr("text-anchor", "middle")
    .attr("x", width / 2)
    .attr("y", marginTop / 2)
    .text("Status of latest audit per block (pre-merge)");

  const tooltip = d3
    .select("body")
    .append("div")
    .attr("id", "audit-block-status-graph-tooltip")
    .style("position", "absolute")
    .style("visibility", "hidden")
    .style("opacity", 0.9)
    .style("background-color", "#fff")
    .style("border-radius", "5px")
    .style("padding", "5px");

  // Append a vertical line to the chart, initially hidden
  const verticalLine = focus
    .append("line")
    .style("stroke", "grey")
    .style("stroke-width", "1px")
    .style("stroke-dasharray", "3,3");

  function computePointedSeries(y0, percentages) {
    let sum = 0;
    const cumsum = keys.map((key) => {
      sum += percentages[key];
      return sum;
    });

    return keys.filter((key, index) => y0 <= cumsum[index])[0];
  }

  // Handle mouse movements
  function mousemove(event) {
    const relX = d3.pointer(event)[0];
    const relY = d3.pointer(event)[1];

    moveTooltip(relX, relY, event.pageX, event.pageY);
  }

  // Handle movement during brush
  function brushing({ sourceEvent }) {
    if (sourceEvent) {
      const relX = sourceEvent.offsetX;
      const relY = sourceEvent.offsetY;

      moveTooltip(relX, relY, sourceEvent.pageX, sourceEvent.pageY);
    }
  }

  function moveTooltip(relX, relY, absX, absY) {
    // Convert from pixels to data
    const x0 = x.invert(relX);
    const y0 = y.invert(relY);

    // Get datapoint under the mouse
    const pointedDatum = data.filter((datapoint) => datapoint.date > x0)[0];

    let pointedSeries = computePointedSeries(y0, pointedDatum.percentages);

    // Prepare tooltip data
    const tooltipTable = d3
      .select(document.createElement("table"))
      .attr("style", "width: 100%");

    series.toReversed().forEach((serie) => {
      const row = tooltipTable.append("tr");

      if (pointedSeries == serie.label) {
        row.attr("class", "fw-bold");
      }

      row.append("td").text("♦").attr("style", `color:${serie.stroke}`);
      row.append("td").text(serie.label).attr("class", "text-capitalize");
      row
        .append("td")
        .text(formatValue(pointedDatum[serie.label]))
        .attr("class", "text-end")
        .attr("style", "padding-left: 1em");
      row
        .append("td")
        .text(formatPercentage(pointedDatum.percentages[serie.label]))
        .attr("class", "text-end")
        .attr("style", "padding-left: 1em");
    });

    // Update the tooltip position
    tooltip
      .style("left", absX + 10 + "px")
      .style("top", absY + 10 + "px")
      .html(`<b>${formatDate(pointedDatum.date)}</b>`)
      .node()
      .appendChild(tooltipTable.node());

    // Update the vertical line position
    verticalLine
      .attr("x1", relX)
      .attr("x2", relX)
      .attr("y1", marginTop)
      .attr("y2", height - marginBottom);
  }

  // Add buttons to the bottom of the legend
  const buttons = svg
    .append("g")
    .attr("transform", `translate(${width - marginRight + 20}, 0)`);

  buttons
    .append("text")
    .attr("class", "reset-button")
    .attr("x", 0)
    .attr("y", 30)
    .attr("font-size", "12px")
    .attr("text-anchor", "start")
    .text("Reset")
    .style("cursor", "pointer")
    .on("click", () => {
      resetChart();
    });

  async function zoomChart({ selection }) {
    if (selection) {
      const start = Math.round(x.invert(selection[0]).getTime() / 1000);
      const end = Math.round(x.invert(selection[1]).getTime() / 1000);

      updateChart(start, end, 500);
    }
  }

  async function resetChart() {
    updateChart(
      tp(GENESIS_TIMESTAMP).getTime() / 1000,
      tp(LAST_MINED_BLOCK_TIMESTAMP).getTime() / 1000,
      0,
    );
  }

  async function updateChart(start, end, transitionDuration) {
    const spinner = new Spinner(spinerOpts).spin(target);

    const contentType = document.getElementById(
      "audit-block-status-content-type",
    ).value;

    data = parseData(
      await d3.json(
        `${BASE_URL}?content_type=${contentType}&start=${start}&end=${end}`,
      ),
      keys,
    );

    context.call(brush.clear);

    x.domain(d3.extent(data, (d) => d.date));
    const filteredStackedData = stacker(data);

    const timedTransition = d3.transition().duration(transitionDuration);

    // Update axis and area position
    xAxis.transition(timedTransition).call(d3.axisBottom(x));

    series = series.map((serie, index) => {
      serie.area
        .transition(timedTransition)
        .attr("fill", serie.fill)
        .attr("d", areaGenerator(filteredStackedData[index]));

      serie.line
        .transition(timedTransition)
        .attr("stroke", serie.stroke)
        .attr("d", lineGenerator(filteredStackedData[index]));

      return { ...serie, data: filteredStackedData[index] };
    });

    spinner.stop();
  }

  await resetChart();
}

export var auditBlockStatusChart = async function () {
  document
    .getElementById("audit-block-status-content-type")
    .addEventListener("change", () => {
      document.getElementById("audit-block-status-graph-svg").remove();
      loadChart();
    });

  loadChart();
};
