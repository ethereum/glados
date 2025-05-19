function radius_stacked_chart(data) {
    // If a node has a larger radius fraction than this, ignore it in the coverage calculation
    const maxRadiusFraction = 0.1;
    const bucket_bit_width = 8;
    const num_buckets = 2 ** bucket_bit_width;
    const margin = {top: 40, right: 15, bottom: 60, left: 65},
        width = 1060 - margin.left - margin.right,
        height = 425 - margin.top - margin.bottom;

    // append the svg object to the body of the page
    const svg = d3.select("#census-stacked")
        .append("svg")
        .attr("width", width + margin.left + margin.right)
        .attr("height", height + margin.top + margin.bottom)
        .append("g")
        .attr("transform",
            `translate(${margin.left}, ${margin.top})`);

    // Indexed data
    // based on shape of data produced in this example: https://d3js.org/d3-shape/stack#_stack
    const full = Array(num_buckets).fill(0);
    const part = Array(num_buckets).fill(0.0);
    data.forEach(function (node, idx, arr) {
      const nodePrefix = Number(BigInt(node.node_id) >> (64n - BigInt(bucket_bit_width)));
      const radiusPrefix = node.radius_top;
      if (radiusPrefix / 256 > maxRadiusFraction) {
        // Ignore nodes with large radius. They are probably new nodes that are syncing
        return ;
      }
      for (let bucket = 0; bucket < num_buckets; bucket++) {
        const distance = nodePrefix ^ bucket;
        if (distance < radiusPrefix) {
          full[bucket] += 1;
        }
        else if (distance == radiusPrefix) {
          part[bucket] += node.radius_lower_fraction;
        }
        // else if distance > prefix, then it shouldn't show any bar, so take no action.
      }
    });
    const compiledData = Array(bucket_bit_width * 2);
    full.forEach((covers, prefix, arr) => compiledData[prefix] = {prefix: prefix, coverage: "full", fraction: covers});
    part.forEach((covers, prefix, arr) => compiledData[num_buckets+prefix] = {prefix: prefix, coverage: "part", fraction: covers});
    const indexedData = d3.index(compiledData, d => d.prefix, d => d.coverage);

    stackedData = d3.stack()
      .keys(["full", "part"])
      .value(function ([, group], key) {
        if (group.has(key)) {
          return group.get(key).fraction;
        } else {
          return 0;
        }
      })
      (indexedData);

    // Add X axis
    const x = d3.scaleLinear()
        .domain([0, num_buckets])
        .range([ 0, width ]);
    const ticks = Array(17).fill(0).map((none, index) => index * 16);
    ticks[ticks.length - 1] -= 1;
    svg.append("g")
        .attr("transform", `translate(0, ${height})`)
        .call(d3.axisBottom(x).tickValues(ticks).tickFormat(d3.format("#0x")))
        .selectAll("text")
        .style("text-anchor", "end")
        .attr("dx", "-.8em")
        .attr("dy", ".15em")
        .attr("transform", "rotate(-55)");
    // X-axis label
    svg.append("text")
        .attr("class", "x label")
        .attr("text-anchor", "middle")
        .attr("x", width / 2)
        .attr("y", height + margin.top + margin.bottom - 15)
        .text("First byte of Content ID");

    // Add Y axis
    const maxBarHeight = d3.max(stackedData, d => d3.max(d, d => d[1]));
    const y = d3.scaleLinear()
        .domain([0, Math.ceil(maxBarHeight)])
        .range([ height, 0]);
    svg.append("g")
        .call(d3.axisLeft(y));
    // Y-axis label
    svg.append("text")
        .attr("class", "y label")
        .attr("text-anchor", "end")
        .attr("y", 0)
        .attr("dy", "-2.5em")
        .attr("transform", "rotate(-90)")
        .text("# of nodes claiming to want content ->>");

    // Add title
    svg.append("text")
        .attr("class", "graph-title")
        .attr("text-anchor", "middle")
        .attr("x", width / 2)
        .attr("y", 0 - (margin.top / 2))
        .text("Content Replication, by Content ID Prefix");

    const hover = d3.select("#hover");

    function hoverAppear(event, d) {
        let coverage = d[1]-d[0];
        const coverageType = d3.select(this.parentNode).datum().key;
        if (coverageType == "part") {
          coverage = coverage.toFixed(2);
        }
        let coverageName;
        if (coverageType == "full") {
          coverageName = "exact";
        } else if (coverageType == "part") {
          coverageName = "statistical average";
        }
        else {
          throw new Error("Unknown coverage type: " + coverageType);
        }
        // try to ID bucket:
        const barx = parseFloat(event.target.getAttribute("x"));
        const bucketnum = Math.round(barx / (width/num_buckets));
        const buckethex = bucketnum.toString(16).padStart(2, '0');
        hover.html(`Data Prefix: 0x${buckethex}<br>Type: ${coverageName}<br>Coverage multiple: ${coverage}`);

        hover
            .style("opacity", 0.9)
            .style("background-color", "#ccc")
            .style("border-radius", "5px");
    }

    function hoverFollow(event, d) {
        const hoverX = event.pageX + 10;
        const hoverY = event.pageY - 10;
        hover
            .style("left", hoverX + "px")
            .style("top", hoverY + "px");
    }

    function hoverGone() {
        hover
            .style("opacity", 0)
            .style("background-color", "transparent")
            .style("border-radius", "0px");
    }


    svg.append('g')
        .selectAll("g")
        .data(stackedData)
        .join("g")
          .attr("fill", function(d) {
            let dark = '#175c50';
            let light = '#31d4b7';
            if (d.key == "full") {
              return dark;
            } else if (d.key == "part") {
              return light;
            }
          })
        .selectAll("rect")
        .data(d => d)
        .join("rect")
          .attr("x", function (d) {
            return x(d.data[0]); } )
          .attr("y", function (d) {
            return y(d[1]); } )
          .attr("height", function (d) {
            return y(d[0]) - y(d[1]);
          })
          .attr("width", width/num_buckets)
          .style("opacity", 0.9)
          .on("mouseover", hoverAppear)
          .on("mousemove", hoverFollow)
          .on("mouseout", hoverGone);
}

function radius_node_id_scatter_chart(data) {
    const margin = {top: 40, right: 2.5, bottom: 50, left: 37},
        width = 475 - margin.left - margin.right,
        height = 485 - margin.top - margin.bottom;

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
        .call(d3.axisLeft(y).tickFormat(d => d + "%"));

    // Add title
    svg.append("text")
        .attr("class", "graph-title")
        .attr("text-anchor", "middle")
        .attr("x", width / 2)
        .attr("y", 0 - (margin.top / 2))
        .text("Radius as % of Keyspace, by Node ID");

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
            let red = '#DA251D'
            let grey = '#808080'
            const clientString = getClientStringFromDecodedEnr(d.raw_enr);
                if (clientString[0] === "f" || clientString[0] === "n") {
                    return blue;
                } else if (clientString[0] === "t") {
                    return purple; 
                } else if (clientString[0] === "u") {
                    return orange; 
                } else if (clientString[0] === "s") {
                  return red; 
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
            if (fullClientString[0] === 'f' || fullClientString[0] === 'n') {
                return "nimbus";
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

