function PieChart(data, {
    name = ([x]) => x,
    value = ([, y]) => y,
    title,
    width = 640,
    height = 400,
    innerRadius = 0,
    outerRadius = Math.min(width, height) / 2,
    labelRadius = (innerRadius * 0.4 + outerRadius * 0.6),
    format = ",",
    names,
    colors,
    stroke = innerRadius > 0 ? "none" : "white",
    strokeWidth = 1,
    strokeLinejoin = "round",
    padAngle = stroke === "none" ? 1 / outerRadius : 0,
} = {}) {
    // Compute values.
    const N = d3.map(data, name);
    const V = d3.map(data, value);
    const I = d3.range(N.length).filter(i => !isNaN(V[i]));

    // Unique the names.
    if (names === undefined) names = N;
    names = new d3.InternSet(names);

    // Chose a default color scheme based on cardinality.
    if (colors === undefined) colors = d3.schemeSpectral[names.size];
    if (colors === undefined) colors = d3.quantize(t => d3.interpolateSpectral(t * 0.8 + 0.1), names.size);

    // Construct scales.
    const color = d3.scaleOrdinal(names, colors);

    // Compute titles.
    if (title === undefined) {
        const formatValue = d3.format(format);
        title = i => `${N[i]}\n${formatValue(V[i])}`;
    } else {
        const O = d3.map(data, d => d);
        const T = title;
        title = i => T(O[i], i, data);
    }

    // Construct arcs.
    const arcs = d3.pie().padAngle(padAngle).sort(null).value(i => V[i])(I);
    const arc = d3.arc().innerRadius(innerRadius).outerRadius(outerRadius);
    const arcLabel = d3.arc().innerRadius(labelRadius).outerRadius(labelRadius);

    const svg = d3.create("svg")
        .attr("width", width)
        .attr("height", height)
        .attr("viewBox", [-width / 2, -height / 2, width, height])
        .attr("style", "max-width: 100%; height: auto; height: intrinsic;");

    svg.append("g")
        .attr("stroke", stroke)
        .attr("stroke-width", strokeWidth)
        .attr("stroke-linejoin", strokeLinejoin)
        .selectAll("path")
        .data(arcs)
        .join("path")
        .attr("fill", d => color(N[d.data]))
        .attr("d", arc)
        .append("title")
        .text(d => title(d.data));

    svg.append("g")
        .attr("font-family", "sans-serif")
        .attr("font-size", 15)
        .attr("text-anchor", "middle")
        .selectAll("text")
        .data(arcs)
        .join("text")
        .attr("transform", d => `translate(${arcLabel.centroid(d)})`)
        .selectAll("tspan")
        .data(d => {
            const lines = `${title(d.data)}`.split(/\n/);
            return (d.endAngle - d.startAngle) > 0.25 ? lines : lines.slice(0, 1);
        })
        .join("tspan")
        .attr("x", 0)
        .attr("y", (_, i) => `${i * 1.1}em`)
        .attr("font-weight", (_, i) => i ? null : "bold")
        .text(d => d);

    return Object.assign(svg.node(), { scales: { color } });
}

function pie_chart_count(client_diversity_data) {
    const char_array = [];

    client_diversity_data.forEach(i => {
        if (i.client_name === "t" || i.client_name === "\\x74") {
            char_array.push({ name: "Trin", value: i.client_count, color: "#9B59B6" });
        } else if (i.client_name === "f" || i.client_name === "\\x66") {
            char_array.push({ name: "Fluffy", value: i.client_count, color: "#3498DB" });
        } else if (i.client_name === "u" || i.client_name === "\\x75") {
            char_array.push({ name: "Ultralight", value: i.client_count, color: "#E67E22" });
        } else {
            char_array.push({ name: "Unknown", value: i.client_count, color: "#808080" });
        }
    });

    const totalValue = char_array.reduce((sum, entry) => sum + entry.value, 0);
    const hasData = totalValue > 0;

    const title = d => {
        if (d.value === 0) {
            return "";
        }
        return hasData ? `${d.name}\n${d3.format(",")(d.value)}` : "";
    };

    const clients = new Set(char_array.map(d => d.name));
    const colors = clients.size > 1 ? char_array.map(d => d.color) : ["white"];

    const chart = PieChart(char_array, {
        name: d => d.name,
        value: d => d.value,
        width: 210,
        height: 210,
        title: title,
        colors: colors
    });

    document.getElementById("graph2").appendChild(chart);
}
