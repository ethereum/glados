import { Spinner, spinnerOpts } from "./spin_conf.js";

const TRANSITION_DURATION = 1000;

const formatDate = d3.utcFormat("%Y-%m-%d %H:%M");
const formatValue = d3.format(",");
const formatPercentage = d3.format(".0%");
const tp = d3.utcParse("%Y-%m-%dT%H:%M:%S%Z");

const dataShapes = {
  long: {
    xAxisProp: "0",
    stacker: (data, keys, keyAttribute, valueAttribute) => {
      return d3
        .stack()
        .keys(keys)
        .value(([, group], key) => {
          if (group.get(key)) return group.get(key)[valueAttribute];
          return null;
        })(
        d3.index(
          data,
          (d) => d.date,
          (d) => d[keyAttribute],
        ),
      );
    },
  },
  wide: {
    xAxisProp: "date",
    stacker: (data, keys) => {
      return d3
        .stack()
        .keys(keys)
        .value((d, key) => d.percentages[key])(data);
    },
  },
  singleSeries: {
    xAxisProp: "date",
    stacker: (data, keys, keyAttribute, valueAttribute) => {
      return d3
        .stack()
        .keys(["single"])
        .value((d) => d[valueAttribute])(
        data.map((d) => {
          return { ...d, key: "single" };
        }),
      );
    },
  },
};

function parseDataNormalLong(rawData) {
  return rawData.map((period) => {
    return {
      ...period,
      date: tp(period.start),
    };
  });
}

function parseDataHundrethWide(rawData, keys) {
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

const getByKeyOrDefault = function (obj, key) {
  return obj[key] ?? obj["other"];
};

/**
 * Customizes how the series are displayed
 *
 * @callback customSeriesDisplayFn
 * @param {Object} seriesMetadata - The array passed in `graphConfig.seriesMetadata` converted into a dictionary with `slug` as keys.
 * @param {string[]} - The slugs of the series
 * @param {Object|null} - The DOM object for the graph's select (if found in the DOM)
 */

/**
 * Exported as areaGraph()
 *
 * Fetches data and generates an area graph
 *
 * @param {Object} graphConfig - Configuration for the graph
 * @param {string} graphConfig.baseName - Base name for the graph, the graph will be appended to a div with the id `${baseName}-graph` which should be found inside another div with id `${baseName}-graph-container`.
 * @param {string} graphConfig.baseUrl - Url to fetch data from, not including any parameters.
 * @param {string} graphConfig.graphTitle - Title for the graph.
 * @param {string} graphConfig.labelY - Label for the y axis, will be shown in the upper left corner.
 * @param {string} graphConfig.kind - Interaction method for the graph. `weekly` allows for requesting and displaying data one week at a time, using a `weeksAgo` parameter, by using buttons. `zoom` allows for requesting and displaying arbitrary time periods, using `start` and `end` parameters, by selecting part of the graph.
 * @param {string} graphConfig.dataShape - Format in which the api returns data options are `singleSeries`, `long` and `wide`.
 * @param {string} graphConfig.valueAttribute - Name of the attribute to use as value in each object from the api response.
 * @param {Object[]} graphConfig.seriesMetadata - List of metadata for each series
 * @param {string} graphConfig.seriesMetadata[].slug - Identifier for the series
 * @param {string} graphConfig.seriesMetadata[].color - Color for the series
 * @param {string} graphConfig.seriesMetadata[].name - Display name for the series
 * @param {Object} graphConfig.initialRanges - Ranges for the graph while fetching data
 * @param {Date[]} graphConfig.initialRanges.x - Array with the first and last date in the x axis
 * @param {number[]} graphConfig.initialRanges.y - Array with the start and end of the y axis range
 * @param {string} [graphConfig.color] - Required if `dataShape === 'singleSeries'`. Color for the single series
 * @param {string} [graphConfig.keyAttribute] - Required if `dataShape === 'long'`. Name of the attribute to use as a series key in each object from the api response.
 * @param {string} [graphConfig.selectParam] - Name of the GET select parameter to call the api with. When set a select with the id `${baseName}-select` will be watch to call the api.
 * @param {string} [graphConfig.stackHundrethPercent=false] - When stacking the series use percentages instead of absolute values.
 * @param {string} [graphConfig.sortDescending=false] - Reverse the alphabetical sorting of series (Ignored when `kind === 'wide'`, in those cases the `seriesMetadata` order is followed).
 * @param {string} [graphConfig.customSeriesDisplayFn] - Customize how the series are displayed.
 */
async function loadChart(graphConfig) {
  const width = 1060;
  const height = 425;
  const marginTop = 40;
  const marginRight = 50;
  const marginBottom = 20;
  const marginLeft = 40;

  // Extract frequently used properties from the config
  const {
    baseName,
    keyAttribute,
    valueAttribute,
    dataShape,
    stackHundrethPercent = false,
    sortDescending = false,
  } = graphConfig;
  const target = document.getElementById(`${baseName}-graph-container`);

  let seriesMetadata = {};
  if (dataShape === "singleSeries") {
    seriesMetadata = { single: { color: graphConfig.color } };
  } else {
    seriesMetadata = Object.fromEntries(
      graphConfig.seriesMetadata.map((s) => [s.slug, s]),
    );
  }
  let seriesProps = {};

  let data = [];
  let keys = [];
  let brush = null;
  let context = null;

  // Declare the x (horizontal position) scale.
  const x = d3.scaleUtc(graphConfig.initialRanges.x, [
    marginLeft,
    width - marginRight,
  ]);

  // Declare the y (vertical position) scale.
  const y = d3.scaleLinear(graphConfig.initialRanges.y, [
    height - marginBottom,
    marginTop,
  ]);

  const areaGenerator = d3
    .area()
    .x((d) => x(d.data[dataShapes[dataShape].xAxisProp]))
    .y0((d) => y(d[0]))
    .y1((d) => y(d[1]));

  // Create the SVG container.
  const svg = d3
    .select(`#${baseName}-graph`)
    .append("svg")
    .attr("width", width)
    .attr("height", height)
    .attr("viewBox", [0, 0, width, height])
    .attr("id", `${baseName}-graph-svg`)
    .attr("style", "max-width: 100%; height: auto; overflow: visible;");

  const dataGroup = svg.append("g");

  const focus = svg.append("g").style("visibility", "hidden");

  // Add the x-axis.
  const xAxis = svg
    .append("g")
    .attr("transform", `translate(0,${height - marginBottom})`)
    .call(d3.axisBottom(x));

  const yTicks = (g) => {
    g.select(".domain").remove();
    g.selectAll(".tick line")
      .attr("x2", width - marginLeft - marginRight)
      .attr("stroke-opacity", 0.1);
  };

  // Add the y-axis, remove the domain line, add grid lines and a label.
  const yAxis = svg
    .append("g")
    .attr("transform", `translate(${marginLeft},0)`)
    .call(
      d3
        .axisLeft(y)
        .tickFormat(stackHundrethPercent ? formatPercentage : formatValue),
    )
    .call(yTicks)
    .call((g) =>
      g
        .append("text")
        .attr("x", -marginLeft)
        .attr("y", 10)
        .attr("fill", "currentColor")
        .attr("text-anchor", "start")
        .text(graphConfig.labelY),
    );

  // Create an overlay for the mouseover event
  if (graphConfig.kind === "weekly") {
    svg
      .append("rect")
      .attr("class", "overlay")
      .attr("x", marginLeft)
      .attr("y", marginTop)
      .attr("width", width - marginLeft - marginRight)
      .attr("height", height - marginTop - marginBottom)
      .style("fill", "none")
      .style("pointer-events", "all")
      .on("mouseover", function () {
        focus.style("visibility", null);
        tooltip.style("visibility", null);
      })
      .on("mouseout", function () {
        focus.style("visibility", "hidden");
        tooltip.style("visibility", "hidden");
      })
      .on("mousemove", mousemove);
  } else if (graphConfig.kind === "zoom") {
    brush = d3
      .brushX()
      .extent([
        [marginLeft, marginTop],
        [width - marginRight, height - marginBottom],
      ])
      .on("end", zoomChart)
      .on("brush", brushing);

    context = svg
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
  }

  // Add title
  svg
    .append("text")
    .attr("class", "graph-title")
    .attr("text-anchor", "middle")
    .attr("x", width / 2)
    .attr("y", marginTop / 2)
    .text(graphConfig.graphTitle);

  const tooltip = d3
    .select("body")
    .append("div")
    .attr("id", `{baseName}-graph-tooltip`)
    .style("position", "absolute")
    .style("visibility", "hidden")
    .style("opacity", 0.9)
    .style("background-color", "#fff")
    .style("border-radius", "5px")
    .style("padding", "5px");

  // Append a vertical line to the graph, initially hidden
  const verticalLine = focus
    .append("line")
    .style("stroke", "grey")
    .style("stroke-width", "1px")
    .style("stroke-dasharray", "3,3");

  function computePointedSeries(y0, pointedDateValues) {
    let sum = 0;
    const cumsum = keys.map((key) => {
      sum += pointedDateValues[key];
      return sum;
    });

    return keys.filter((key, index) => y0 <= cumsum[index])[0];
  }

  // Handle mouse movements
  function mousemove(event) {
    if (data.length > 0) {
      const relX = d3.pointer(event)[0];
      const relY = d3.pointer(event)[1];

      moveTooltip(relX, relY, event.pageX, event.pageY);
    }
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
    let newerData = data.filter((datapoint) => datapoint.date >= x0);
    if (newerData.length === 0) {
      return;
    }
    const pointedDatum = newerData[0];

    let pointedDateValues = {};
    if (stackHundrethPercent) {
      pointedDateValues = pointedDatum.percentages;
    } else {
      const pointedDateData = data.filter((d) => d.start == pointedDatum.start);
      pointedDateValues = Object.fromEntries(
        keys.map((key) => [
          key,
          (pointedDateData.filter((d) => d[keyAttribute] === key)[0] ??
            Object.fromEntries([[valueAttribute, 0]]))[valueAttribute],
        ]),
      );
    }

    const pointedSeries = computePointedSeries(y0, pointedDateValues);

    // Prepare tooltip data
    const tooltipTable = d3
      .select(document.createElement("table"))
      .attr("style", "width: 100%");

    if (dataShape === "singleSeries") {
      const row = tooltipTable.append("tr");
      row.append("td").text(formatValue(pointedDatum[valueAttribute]));
    } else {
      keys.toReversed().forEach((key) => {
        const row = tooltipTable.append("tr");

        if (pointedSeries == key) {
          row.attr("class", "fw-bold");
        }

        let keyProps = getByKeyOrDefault(seriesProps, key);
        row.append("td").text("♦").attr("style", `color:${keyProps.color}`);
        row.append("td").text(keyProps.name);

        if (stackHundrethPercent) {
          row
            .append("td")
            .text(formatValue(pointedDatum[key]))
            .attr("class", "text-end")
            .attr("style", "padding-left: 1em");
          row
            .append("td")
            .text(formatPercentage(pointedDatum.percentages[key]))
            .attr("class", "text-end")
            .attr("style", "padding-left: 1em");
        } else {
          row
            .append("td")
            .text(formatValue(pointedDateValues[key]))
            .attr("class", "text-end")
            .attr("style", "padding-left: 1em");
        }
      });
    }

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

  if (graphConfig.kind === "weekly") {
    // Add the previous button ("<")
    buttons
      .append("text")
      .attr("class", "previous-button")
      .attr("x", 0)
      .attr("y", 30)
      .attr("font-size", "12px")
      .attr("text-anchor", "start")
      .text("< Previous")
      .style("cursor", "pointer");

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
      .style("visibility", "hidden");
  } else if (graphConfig.kind === "zoom") {
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
  }

  async function zoomChart({ selection }) {
    if (selection) {
      const start = Math.round(x.invert(selection[0]).getTime() / 1000);
      const end = Math.round(x.invert(selection[1]).getTime() / 1000);

      updateChart({ start, end }, TRANSITION_DURATION);
    }
  }

  async function resetChart() {
    const period =
      graphConfig.kind === "weekly"
        ? { weeksAgo: 0 }
        : {
            start: graphConfig.initialRanges.x[0].getTime() / 1000,
            end: graphConfig.initialRanges.x[1].getTime() / 1000,
          };

    updateChart(period, 0);
  }

  async function updateChart(period, transitionDuration) {
    const spinner = new Spinner(spinnerOpts).spin(target);

    const url = new URL(window.location);
    const subprotocol = url.searchParams.get("subprotocol")
      ? url.searchParams.get("subprotocol").toLowerCase()
      : "history";

    const params = new URLSearchParams();
    params.append("subprotocol", subprotocol);

    if (graphConfig.kind === "weekly") {
      params.append("weeks-ago", period.weeksAgo);
    } else if (graphConfig.kind === "zoom") {
      params.append("start", period.start);
      params.append("end", period.end);
    }

    const select = document.getElementById(`${baseName}-select`);
    if (graphConfig.selectParam) {
      params.append(graphConfig.selectParam, select.value);
    }

    const rawData = await d3.json(graphConfig.baseUrl + params);

    if (dataShape === "singleSeries" && !stackHundrethPercent) {
      data = parseDataNormalLong(rawData);
    } else if (dataShape === "long" && !stackHundrethPercent) {
      data = parseDataNormalLong(rawData);
    } else if (stackHundrethPercent && dataShape === "wide") {
      data = parseDataHundrethWide(
        rawData,
        //graphConfig.seriesMetadata.map((s) => s.slug),
        Object.keys(seriesMetadata),
      );
    } else {
      console.error(
        `parseData function not implemented for stackHundrethPercent: ${stackHundrethPercent} dataShape: ${dataShape} graphs`,
      );
    }

    if (dataShape === "long") {
      keys = [...new Set(data.map((d) => d[keyAttribute]))].toSorted();
      if (!sortDescending) {
        keys.reverse();
      }
    } else if (dataShape === "wide") {
      keys = Object.keys(seriesMetadata);
    }

    seriesProps =
      "customSeriesDisplayFn" in graphConfig
        ? graphConfig.customSeriesDisplayFn(seriesMetadata, keys, select)
        : seriesMetadata;

    // Add `other` only if data contains a key not found in seriesMetadata
    if (
      !("other" in seriesMetadata) &&
      keys.some((k) => !(k in seriesMetadata))
    ) {
      seriesProps["other"] = { color: "#808080", name: "Other" };
    }

    if (graphConfig.kind === "zoom") {
      context.call(brush.clear);
    }

    const stackedData = dataShapes[dataShape].stacker(
      data,
      keys,
      keyAttribute,
      valueAttribute,
    );

    const timedTransition = d3.transition().duration(transitionDuration);

    // If chart is weekly, force the domain to the full week instead of sizing to data
    if (graphConfig.kind === "weekly") {
      const currentTime = Date.now();
      const start =
        currentTime - (period.weeksAgo + 1) * 7 * 24 * 60 * 60 * 1000;
      const end = currentTime - period.weeksAgo * 7 * 24 * 60 * 60 * 1000;

      x.domain([start, end]);
    } else {
      x.domain(d3.extent(data, (d) => d.date));
    }

    y.domain([0, d3.max(stackedData, (d) => d3.max(d, (d) => d[1]))]);

    // Update axis and area position
    xAxis.transition(timedTransition).call(d3.axisBottom(x));
    yAxis
      .transition(timedTransition)
      .call(
        d3
          .axisLeft(y)
          .tickFormat(stackHundrethPercent ? formatPercentage : formatValue),
      )

      .call(yTicks);

    dataGroup
      .selectAll("path")
      .data(stackedData)
      .join("path")
      .transition(timedTransition)
      .attr("fill", (d) => getByKeyOrDefault(seriesProps, d.key).color)
      .attr("d", areaGenerator);

    if (graphConfig.kind === "weekly") {
      buttons.select(".previous-button").on("click", () => {
        updateChart({ weeksAgo: period.weeksAgo + 1 }, TRANSITION_DURATION);
      });
      buttons
        .select(".next-button")
        .style("visibility", period.weeksAgo > 0 ? null : "hidden")
        .on("click", () => {
          updateChart({ weeksAgo: period.weeksAgo - 1 }, TRANSITION_DURATION);
        });
    }

    spinner.stop();
  }

  await resetChart();
}

export var areaGraph = async function (graphConfig) {
  if (graphConfig.selectParam) {
    document
      .getElementById(`${graphConfig.baseName}-select`)
      .addEventListener("change", () => {
        document.getElementById(`${graphConfig.baseName}-graph-svg`).remove();
        loadChart(graphConfig);
      });
  }

  loadChart(graphConfig);
};
