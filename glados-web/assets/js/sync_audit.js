function syncAudit() {
    const MERGE_BLOCK = 15_537_393;
    const DEFAULT_SEGMENT_SIZE = 100_000;
    const BAR_WIDTH = 8;

    const EXPECTED_BARS = Math.ceil(MERGE_BLOCK / DEFAULT_SEGMENT_SIZE);

    const LEFT_OFFSET = (window.innerWidth - (EXPECTED_BARS * BAR_WIDTH)) / 2;

    const margin = { top: 20, right: 20, bottom: 70, left: LEFT_OFFSET};
    const height = window.innerHeight - margin.top - margin.bottom;

    const tooltip = d3.select("#tooltip");

    async function drawChart() {
        const response = await fetch("/api/sync-audit-json/");
        const data = (await response.json()).records;

        const segmentSize = data[0]?.segment_end - data[0]?.segment_start + 1 || DEFAULT_SEGMENT_SIZE;
        const totalSegments = Math.ceil(MERGE_BLOCK / segmentSize);
        const totalWidth = totalSegments * BAR_WIDTH;

        const expectedData = Array.from({ length: totalSegments }, (_, i) => {
            const start = i * segmentSize;
            const end = Math.min(MERGE_BLOCK, start + segmentSize - 1);
            const existing = data.find(d => d.segment_start === start);
            return existing || {
                segment_start: start,
                segment_end: end,
                mean_ms: 0,
                median_ms: 0,
                p99_ms: 0,
                min_ms: 0,
                max_ms: 0,
                num_errors: 0,
                in_progress: true
            };
        });

        const x = d3.scaleLinear()
            .domain([0, expectedData.length])
            .range([0, totalWidth]);

        const y = d3.scaleLinear()
            .domain([0, d3.max(expectedData, d => d.median_ms || 1000)])
            .nice()
            .range([height, 0]);

        const svg = d3.select("#chart")
            .html("")
            .append("svg")
            .attr("width", totalWidth + margin.left + margin.right)
            .attr("height", height + margin.top + margin.bottom)
            .append("g")
            .attr("transform", `translate(${margin.left},${margin.top})`);

        const xAxis = d3.axisBottom(x)
            .tickValues(d3.range(0, totalSegments, Math.ceil(totalSegments / 20)))
            .tickFormat(i => expectedData[i]?.segment_start.toLocaleString() || "");

        const yAxis = d3.axisLeft(y);

        svg.append("g")
            .attr("transform", `translate(0,${height})`)
            .call(xAxis)
            .selectAll("text")
            .attr("transform", "rotate(-45)")
            .style("text-anchor", "end");

        svg.append("g").call(yAxis);

        svg.append("text")
            .attr("class", "axis-label")
            .attr("x", totalWidth / 2)
            .attr("y", height + 60)
            .style("text-anchor", "middle")
            .text("Segment Start Block");

        svg.append("text")
            .attr("class", "axis-label")
            .attr("transform", "rotate(-90)")
            .attr("x", -height / 2)
            .attr("y", -40)
            .style("text-anchor", "middle")
            .text("Median Response Time (ms)");

        const bars = svg.selectAll(".bar")
            .data(expectedData)
            .enter()
            .append("rect")
            .attr("class", d => d.in_progress ? "bar in-progress" : "bar")
            .attr("fill", "#1f77b4")
            .attr("x", (_, i) => x(i))
            .attr("y", d => y(d.median_ms || 0))
            .attr("width", BAR_WIDTH)
            .attr("height", d => height - y(d.median_ms || 0))
            .on("mouseover", function (event, d, i) {
                d3.select(this).attr("fill", "#72b3f0");

                tooltip.transition().duration(200).style("opacity", 0.9);
                tooltip
                    .html(d.in_progress
                        ? `<strong>Segment:</strong> ${d.segment_start.toLocaleString()}–${d.segment_end.toLocaleString()}<br/>In progress...`
                        : `<strong>Block:</strong> ${d.segment_start.toLocaleString()}–${d.segment_end.toLocaleString()}<br/>
                            <strong>Mean:</strong> ${d.mean_ms.toLocaleString()} ms<br/>
                            <strong>Median:</strong> ${d.median_ms.toLocaleString()} ms<br/>
                            <strong>P99:</strong> ${d.p99_ms.toLocaleString()} ms<br/>
                            <strong>Min/Max:</strong> ${d.min_ms.toLocaleString()} / ${d.max_ms.toLocaleString()} ms<br/>
                            <strong>Errors:</strong> ${d.num_errors.toLocaleString()}`)
                    .style("left", (event.pageX + 15) + "px")
                    .style("top", (event.pageY - 30) + "px");
            })
            .on("mousemove", function (event, d) {
                tooltip.style("left", (event.pageX + 15) + "px")
                       .style("top", (event.pageY - 30) + "px");
            })
            .on("mouseout", function () {
                d3.select(this).attr("fill", "#1f77b4");
                tooltip.transition().duration(500).style("opacity", 0);
            })
            .on("click", function (_, d, i) {
                const allBars = d3.selectAll(".bar").nodes();
                const next = allBars[i + 1];
                if (next) next.scrollIntoView({ behavior: "smooth", block: "center", inline: "center" });
            });
    }

    drawChart();
}