function createMultiLineChart(height, width, dataSets) {
    // Declare the chart dimensions and margins.
    const marginTop = 20;
    const marginRight = 50;
    const marginBottom = 20;
    const marginLeft = 40;

    // Parse dates if they're not already Date objects
    dataSets.forEach(dataset => {
        dataset.forEach(d => {
            if (!(d.date instanceof Date)) d.date = new Date(d.date);
        });
    });

    // Declare the x (horizontal position) scale.
    const x = d3.scaleTime()
        .domain(d3.extent(dataSets.flat(), d => d.date))
        .range([marginLeft, width - marginRight]);

    // Declare the y (vertical position) scale.
    const y = d3.scaleLinear()
        .domain([0, d3.max(dataSets.flat(), d => d.value)])
        .range([height - marginBottom, marginTop]);

    // Color palette for the lines.
    const colors = d3.schemeTableau10;

    // Array to keep track of which datasets are visible.
    let visibility = new Array(dataSets.length).fill(true);

    // Create the SVG container.
    const svg = d3.create("svg")
        .attr("width", width)
        .attr("height", height)
        .attr("viewBox", [0, 0, width, height])
        .attr("overflow", "visible")
        .attr("style", "max-width: 100%; height: auto; height: intrinsic;");

    // Add the x-axis.
    svg.append("g")
        .attr("transform", `translate(0,${height - marginBottom})`)
        .call(d3.axisBottom(x).ticks(width / 80).tickSizeOuter(0));

    // Add the y-axis, remove the domain line, add grid lines and a label.
    svg.append("g")
        .attr("transform", `translate(${marginLeft},0)`)
        .call(d3.axisLeft(y).ticks(height / 40))
        .call(g => g.select(".domain").remove())
        .call(g => g.selectAll(".tick line").clone()
            .attr("x2", width - marginLeft - marginRight)
            .attr("stroke-opacity", 0.1))
        .call(g => g.append("text")
            .attr("x", -marginLeft)
            .attr("y", 10)
            .attr("fill", "currentColor")
            .attr("text-anchor", "start")
            .text("↑ Success Rate"));

    // Add lines to the graph.
    const lines = dataSets.map((dataSet, i) => {
        const line = d3.line()
            .defined(d => !isNaN(d.value))
            .x(d => x(d.date))
            .y(d => y(d.value));

        return svg.append("path")
            .datum(dataSet)
            .attr("fill", "none")
            .attr("stroke", colors[i % colors.length])
            .attr("stroke-width", 1.5)
            .attr("d", line)
            .attr("class", `line line-${i}`);
    });

    // Add a legend.
    const legend = svg.selectAll(".legend")
        .data(dataSets)
        .enter().append("g")
        .attr("class", "legend")
        .attr("transform", (d, i) => `translate(${width - marginRight + 100}, ${i * 20})`) // Position adjusted to the right
        .style("font", "10px sans-serif")
        .style("cursor", "pointer")
        .on("click", function (event, d) {
            const index = dataSets.indexOf(d);
            visibility[index] = !visibility[index];
            d3.select(lines[index].node()).style("opacity", visibility[index] ? 1 : 0);
            d3.select(this).style("opacity", visibility[index] ? 1 : 0.5); // Adjust the legend item's opacity
        });

    // Function to dispatch a click event
    function dispatchClick(element) {
        const clickEvent = new MouseEvent('click', {
            'view': window,
            'bubbles': true,
            'cancelable': true
        });
        element.dispatchEvent(clickEvent);
    }

    // Select all '.legend' group elements and click on all but the first three
    svg.selectAll(".legend")
        .each(function (d, i) {
            if (i >= 3) { // Skip the first three
                dispatchClick(this);
            }
        });

    // Add colored rectangles to the legend.
    legend.append("rect")
        .attr("x", -18)
        .attr("width", 18)
        .attr("height", 18)
        .attr("fill", (d, i) => colors[i % colors.length]);

    // Add text to the legend.
    const labels = ["All", "Latest", "Random", "Oldest", "4444s",
        "All Headers", "All Bodies", "All Receipts",
        "Latest Headers", "Latest Bodies", "Latest Receipts",
        "Random Headers", "Random Bodies", "Random Receipts",
        "4444s Headers", "4444s Bodies", "4444s Receipts"];
    legend.append("text")
        .attr("x", -24)
        .attr("y", 9)
        .attr("dy", ".35em")
        .attr("text-anchor", "end")
        .text((d, i) => labels[i])
        .attr("class", "legend-text");

    return svg.node();
}

function convertDataForChart(data) {
    const successRateKeys = [
        'success_rate_all',
        'success_rate_latest',
        'success_rate_random',
        'success_rate_oldest',
        'success_rate_premerge',
        'success_rate_all_headers',
        'success_rate_all_bodies',
        'success_rate_all_receipts',
        'success_rate_latest_headers',
        'success_rate_latest_bodies',
        'success_rate_latest_receipts',
        'success_rate_random_headers',
        'success_rate_random_bodies',
        'success_rate_random_receipts',
        'success_rate_premerge_headers',
        'success_rate_premerge_bodies',
        'success_rate_premerge_receipts'
    ];

    return successRateKeys.map(key =>
        data.map(d => ({
            date: new Date(d.timestamp),
            value: d[key]
        }))
    );
}

// Fetch the stats records from the API.
function getStatsRecords() {
    const baseUrl = "api/stat-history/";

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

// Create the stats history chart using data from the API and add it to the DOM.
async function statsHistoryChart() {
    try {
        const data = await getStatsRecords();
        console.log(data);

        let dataSets = convertDataForChart(data);
        console.log(dataSets)
        if (dataSets && dataSets.length > 0) {
            document.getElementById('stats-history-graph').appendChild(createMultiLineChart(400, 670, dataSets));
        } else {
            console.log('No data available to plot the stats chart');
        }
    } catch (error) {
        console.error('There was an error processing your request:', error.message);
    }
}
