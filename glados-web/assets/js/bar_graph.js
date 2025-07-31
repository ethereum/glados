import { Spinner, spinnerOpts } from "./spin_conf.js";

const TRANSITION_DURATION = 1000;

const formatValue = d3.format(",");

async function loadChart(graphConfig) {
  const width = 1060;
  const height = 425;
  const marginTop = 40;
  const marginRight = 50;
  const marginBottom = 20;
  const marginLeft = 40;

  // Extract frequently used properties from the config
  const { baseName } = graphConfig;
  const target = document.getElementById(`${baseName}-graph-container`);

  const seriesProps = Object.fromEntries(
    graphConfig.seriesMetadata.map((s) => [s.slug, s]),
  );

  let data = [];

  // Declare the x (horizontal position) scale.
  const x = d3.scaleLinear([0, 100], [marginLeft, width - marginRight]);

  // Declare the y (vertical position) scale.
  const y = d3
    .scaleBand()
    .domain([])
    .range([height - marginBottom, marginTop])
    .padding([0.2]);

  // Another scale for positions within a grout
  const ySubscale = d3
    .scaleBand()
    .domain([])
    .range([0, y.bandwidth()])
    .padding([0.05]);

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
  const labelGroup = svg.append("g");

  const xTicks = (g) => {
    g.select(".domain").remove();
    g.selectAll(".tick line")
      .attr("y2", -(height - marginBottom - marginTop))
      .attr("stroke-opacity", 0.1);
  };

  // Add the x-axis.
  const xAxis = svg
    .append("g")
    .attr("transform", `translate(0,${height - marginBottom})`)
    .call(d3.axisBottom(x))
    .call(xTicks)
    .call((g) =>
      g
        .append("text")
        .attr("x", width + marginLeft)
        .attr("y", marginBottom)
        .attr("fill", "currentColor")
        .attr("text-anchor", "end")
        .text(graphConfig.labelX),
    );

  const yTicks = (g) => {
    g.selectAll(".tick line").attr("display", "none");
    g.selectAll(".tick text")
      .style("text-anchor", "middle")
      .attr("dy", "-.4em")
      .attr("dx", "1em")
      .attr("transform", "rotate(-90)");
  };

  // Add the y-axis, remove the domain line, add grid lines and a label.
  const yAxis = svg
    .append("g")
    .attr("transform", `translate(${marginLeft},0)`)
    .call(d3.axisLeft(y))
    .call((g) =>
      g
        .append("text")
        .attr("x", -marginLeft)
        .attr("y", 10)
        .attr("fill", "currentColor")
        .attr("text-anchor", "start")
        .text(graphConfig.labelY),
    )
    .call(yTicks);

  // Add title
  svg
    .append("text")
    .attr("class", "graph-title")
    .attr("text-anchor", "middle")
    .attr("x", width / 2)
    .attr("y", marginTop / 2)
    .text(graphConfig.graphTitle);

  async function updateChart() {
    const spinner = new Spinner(spinnerOpts).spin(target);

    const url = new URL(window.location);
    const subprotocol = url.searchParams.get("subprotocol")
      ? url.searchParams.get("subprotocol").toLowerCase()
      : "history";

    const params = new URLSearchParams();
    params.append("subprotocol", subprotocol);

    const rawData = await d3.json(graphConfig.baseUrl + params);

    let groups = Object.keys(rawData).toSorted().reverse();

    let subgroups = [
      ...new Set(Object.values(rawData).flatMap((d) => Object.keys(d))),
    ].toSorted();

    data = Object.entries(rawData).map(([k, v]) => {
      return {
        name: k,
        children: subgroups.map((key) => {
          return { key: key, value: v[key] ?? 0 };
        }),
      };
    });

    const timedTransition = d3.transition().duration(TRANSITION_DURATION);

    x.domain([
      0,
      d3.max(Object.values(rawData).flatMap((v) => Object.values(v))),
    ]);
    y.domain(groups);
    ySubscale.domain(subgroups).range([0, y.bandwidth()]);

    // Update axis
    xAxis.transition(timedTransition).call(d3.axisBottom(x)).call(xTicks);
    yAxis.transition(timedTransition).call(d3.axisLeft(y)).call(yTicks);

    dataGroup
      .selectAll("g")
      // Enter in data = loop group per group
      .data(data)
      .join("g")
      .attr("transform", (d) => {
        return `translate(0, ${y(d.name)})`;
      })
      .selectAll("rect")
      .data((d) => d.children)
      .join("rect")
      .attr("x", (d) => marginLeft)
      .attr("y", (d) => ySubscale(d.key))
      .attr("width", (d) => x(d.value) - marginLeft)
      .attr("height", ySubscale.bandwidth())
      .attr("fill", (d) => seriesProps[d.key].color);

    labelGroup
      .selectAll("g")
      // Enter in data = loop group per group
      .data(data)
      .join("g")
      .attr("transform", (d) => {
        return `translate(0, ${y(d.name)})`;
      })
      .selectAll("text")
      .data((d) => d.children)
      .join("text")
      .attr("x", (d) => x(d.value) + 8)
      .attr("y", (d) => ySubscale(d.key) + 15)
      .attr("fill", (d) => seriesProps[d.key].color)
      .text((d) => formatValue(d.value));

    // Add a legend.
    const legend = svg
      .selectAll(".legend")
      .data(subgroups)
      .enter()
      .append("g")
      .attr("class", "legend")
      .attr(
        "transform",
        (d, i) => `translate(${width - marginRight + 100}, ${i * 20 + 30})`,
      ) // Position adjusted to the right
      .style("font", "10px sans-serif");

    // Add colored rectangles to the legend.
    legend
      .append("rect")
      .attr("x", -18)
      .attr("width", 18)
      .attr("height", 18)
      .attr("fill", (d) => seriesProps[d].color);

    // Add text to the legend.
    const formatDate = d3.utcFormat("%Y-%m-%d %H:%M");
    legend
      .append("text")
      .attr("x", -24)
      .attr("y", 9)
      .attr("dy", ".35em")
      .attr("text-anchor", "end")
      .text((d) => seriesProps[d].name)
      .attr("class", "legend-text");

    spinner.stop();
  }
  updateChart();
}

export var barGraph = async function (graphConfig) {
  loadChart(graphConfig);
};
