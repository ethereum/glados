function radius_node_id_scatter_chart(data) {
    const margin = {top: 10, right: 2.5, bottom: 50, left: 25},
        width = 475 - margin.left - margin.right,
        height = 425 - margin.top - margin.bottom;

    // append the svg object to the body of the page
    const svg = d3.select("#census-scatterplot")
        .append("svg")
        .attr("width", width + margin.left + margin.right)
        .attr("height", height + margin.top + margin.bottom)
        .append("g")
        .attr("transform",
            `translate(${margin.left}, ${margin.top})`);

    // Add X axis
    const x = d3.scaleLinear()
        .domain([0, 18446744073709551615])
        .range([ 0, width ]);
    svg.append("g")
        .attr("transform", `translate(0, ${height})`)
        .call(d3.axisBottom(x).ticks(10, "e"))
        .selectAll("text")
        .style("text-anchor", "end")
        .attr("dx", "-.8em")
        .attr("dy", ".15em")
        .attr("transform", "rotate(-55)");

    // Add Y axis
    const y = d3.scaleLinear()
        .domain([0, 100])
        .range([ height, 0]);
    svg.append("g")
        .call(d3.axisLeft(y));

    const hover = d3.select("#hover");

    function hoverAppear() {
        hover.transition()
            .style("opacity", 0.9)
            .style("background-color", "#ccc")
            .style("border-radius", "5px");
    }

    function hoverFeature(event, d) {
        const hoverX = event.pageX + 10;
        const hoverY = event.pageY - 10;
        let latestClientString = getClientStringFromDecodedEnr(d.raw_enr);
        hover.html(`Client Name: ${latestClientString}<br>Node ID: ${d.node_id_string}<br>Data Radius: ${d.data_radius}%`)
            .style("left", hoverX + "px")
            .style("top", hoverY + "px")
            .style("background-color", "#ccc")
            .style("border-radius", "5px");
    }

    function hoverGone() {
        hover.transition()
            .style("opacity", 0)
            .style("background-color", "transparent")
            .style("border-radius", "0px");
    }

    // Add dots
    svg.append('g')
        .selectAll("dot")
        .data(data)
        .enter()
        .append("circle")
        .attr("cx", function (d) { return x(d.node_id); } )
        .attr("cy", function (d) { return y(d.data_radius); } )
        .attr("r", 4)
        .style("opacity", 0.9)
        .style("stroke", "white")
        .attr("fill", function(d) {
            let blue = '#3498DB'
            let purple = '#9B59B6'
            let orange = '#E67E22'
            let grey = '#808080'
            const clientString = getClientStringFromDecodedEnr(d.raw_enr);
                if (clientString[0] === "f") {
                    return blue;
                } else if (clientString[0] === "t") {
                    return purple; 
                } else if (clientString[0] === "u") {
                    return orange; 
                } else {
                    return grey; 
                }
        })
        .on("mouseover", hoverAppear)
        .on("mousemove", hoverFeature)
        .on("mouseout", hoverGone);

}

function getClientStringFromDecodedEnr(enr) {
    for (let [key, value] of ENR.ENR.decodeTxt(enr).enr.entries()) {

        if (key === "c") {
            let fullClientString = String.fromCharCode.apply(null, value);
            if (fullClientString[0] === 'f') {
                return "fluffy";
            }
            else if (fullClientString[0] === 'u') {
                return "ultralight";
            }
            else if (fullClientString[0] === 't') {
                clientName = "trin ";
                clientName += fullClientString.substring(2);
                return clientName;
            } else {
                return fullClientString;
            }        
        } else {
            return "unknown";
        }
    }
}

