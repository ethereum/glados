import { Spinner, spinnerOpts } from "./spin_conf.js";

let customSpinnerOpts = {
  ...spinnerOpts,
  width: 30,
  radius: 25,
};

let summaryController = null;
let listController = null;

function shorten(hex) {
  return `${hex.substring(0, 6)}...${hex.substring(hex.length - 4)}`;
}

function formatTimeAgo(dateStr) {
  const YEAR_MS = 365 * 24 * 60 * 60 * 1000;
  const DAY_MS = 24 * 60 * 60 * 1000;
  const HOUR_MS = 60 * 60 * 1000;
  const MINUTE_MS = 60 * 1000;
  const SECOND_MS = 1000;

  const date = Date.parse(dateStr);
  const delta = Date.now() - date;

  const years = Math.floor(delta / YEAR_MS);
  const days = Math.floor((delta - years * YEAR_MS) / DAY_MS);
  const hours = Math.floor((delta - years * YEAR_MS - days * DAY_MS) / HOUR_MS);
  const minutes = Math.floor(
    (delta - years * YEAR_MS - days * DAY_MS - hours * HOUR_MS) / MINUTE_MS,
  );
  const seconds = Math.floor(
    (delta -
      years * YEAR_MS -
      days * DAY_MS -
      hours * HOUR_MS -
      minutes * MINUTE_MS) /
      SECOND_MS,
  );

  let formated = "";
  if (years > 0) {
    formated += `${years}y`;
  }
  if (days > 0) {
    formated += `${days}d`;
  }
  if (hours > 0) {
    formated += `${hours}h`;
  }
  if (minutes > 0) {
    formated += `${minutes}m`;
  }
  // Don't show seconds after a day
  if ((seconds > 0 || formated == "") && days == 0 && years == 0) {
    formated += `${seconds}s`;
  }

  return formated + " ago";
}

function updateSummary(queryString) {
  const auditListSummary = document.getElementById("audit-summary");

  if (summaryController) {
    summaryController.abort("Changed filters");
  }

  summaryController = new AbortController();
  let signal = summaryController.signal;

  auditListSummary.tBodies[0].innerHTML = "";
  const spinner = new Spinner(customSpinnerOpts).spin(auditListSummary);

  fetch(`/api/audits-stats/?${queryString}`, { signal })
    .then((response) => {
      if (!response.ok) {
        throw new Error("Network response was not ok");
      }
      return response.json();
    })
    .then((data) => {
      for (const periodStats of data) {
        const row = document.createElement("tr");

        const period = document.createElement("td");
        period.innerHTML = periodStats.period;
        row.appendChild(period);

        const totalAudits = document.createElement("td");
        totalAudits.innerHTML = periodStats.total_audits.toLocaleString();
        row.appendChild(totalAudits);

        const totalPasses = document.createElement("td");
        totalPasses.innerHTML = periodStats.total_passes.toLocaleString();
        row.appendChild(totalPasses);

        const totalFailures = document.createElement("td");
        totalFailures.innerHTML = periodStats.total_failures.toLocaleString();
        row.appendChild(totalFailures);

        const passRate = document.createElement("td");
        passRate.innerHTML = periodStats.pass_percent.toFixed(1) + "%";
        row.appendChild(passRate);

        const failRate = document.createElement("td");
        failRate.innerHTML = periodStats.fail_percent.toFixed(1) + "%";
        row.appendChild(failRate);

        const auditsPerMinute = document.createElement("td");
        auditsPerMinute.innerHTML =
          periodStats.audits_per_minute.toLocaleString();
        row.appendChild(auditsPerMinute);

        auditListSummary.tBodies[0].appendChild(row);
      }
    })
    .catch((error) => {
      console.error("There was a problem with the fetch operation:", error);
    })
    .finally(() => {
      listController = null;
      spinner.stop();
    });
}
function updateList(queryString) {
  const auditListTable = document.getElementById("audit-list");

  if (listController) {
    listController.abort("Changed filters");
  }

  listController = new AbortController();
  let signal = listController.signal;

  auditListTable.tBodies[0].innerHTML = "";
  const spinner = new Spinner(customSpinnerOpts).spin(auditListTable);

  fetch(`/api/audits/?${queryString}`, { signal })
    .then((response) => {
      if (!response.ok) {
        throw new Error("Network response was not ok");
      }
      return response.json();
    })
    .then((data) => {
      for (const audit of data) {
        const row = document.createElement("tr");

        const id = document.createElement("td");
        if (audit.has_trace) {
          const idAnchor = document.createElement("a");
          idAnchor.href = `/audit/id/${audit.id}`;
          idAnchor.innerHTML = audit.id;
          id.appendChild(idAnchor);
        } else {
          id.innerHTML = audit.id;
        }
        row.appendChild(id);

        const result = document.createElement("td");
        const resultSpan = document.createElement("span");
        resultSpan.classList = ["badge"];
        if (audit.is_success) {
          resultSpan.classList.add("text-bg-success");
          resultSpan.innerHTML = "Success";
        } else {
          resultSpan.classList.add("text-bg-danger");
          resultSpan.innerHTML = "Fail";
        }
        resultSpan.innerHTML = audit.is_success ? "Success" : "Fail";
        result.appendChild(resultSpan);
        row.appendChild(result);

        const contentType = document.createElement("td");
        contentType.innerHTML = audit.content_type;
        row.appendChild(contentType);

        const strategy = document.createElement("td");
        strategy.innerHTML = audit.strategy;
        row.appendChild(strategy);

        const contentKey = document.createElement("td");
        const contentKeyAnchor = document.createElement("a");
        contentKeyAnchor.href = `/content/key/${audit.content_key}/`;
        contentKeyAnchor.innerHTML = shorten(audit.content_key);
        contentKey.appendChild(contentKeyAnchor);
        row.appendChild(contentKey);

        const contentId = document.createElement("td");
        const contentIdAnchor = document.createElement("a");
        contentIdAnchor.href = `/content/id/${audit.content_id}/`;
        contentIdAnchor.innerHTML = shorten(audit.content_id);
        contentId.appendChild(contentIdAnchor);
        row.appendChild(contentId);

        const firstAvailable = document.createElement("td");
        firstAvailable.title = audit.content_available_at;
        firstAvailable.innerHTML = formatTimeAgo(audit.content_available_at);
        row.appendChild(firstAvailable);

        const auditedAt = document.createElement("td");
        auditedAt.title = audit.audited_at;
        auditedAt.innerHTML = formatTimeAgo(audit.audited_at);
        row.appendChild(auditedAt);

        const client = document.createElement("td");
        client.innerHTML = audit.client_version_info;
        row.appendChild(client);

        auditListTable.tBodies[0].appendChild(row);
      }
    })
    .catch((error) => {
      console.error(
        "There was a problem with the fetch operation:",
        error.message,
      );
    })
    .finally(() => {
      listController = null;
      spinner.stop();
    });
}

function updateDashboard(network, strategy, contentType, auditResult) {
  const params = {
    network: network,
  };
  if (strategy) {
    params.strategy = strategy;
  }
  if (contentType) {
    params.content_type = contentType;
  }
  if (auditResult) {
    params.audit_result = auditResult;
  }

  const queryString = new URLSearchParams(params).toString();

  updateList(queryString);
  updateSummary(queryString);
}

export var initAuditDashboard = async function () {
  const network = new URL(window.location).searchParams
    .get("network")
    .toLowerCase();
  const contentGroup = document.querySelector("#content-buttons");
  const strategyGroup = document.querySelector("#strategy-buttons");
  const auditResultGroup = document.querySelector("#audit-result-buttons");

  const activateButton = (btn, group) => {
    // Deactivate all buttons in the group
    group.querySelectorAll(".btn").forEach((button) => {
      button.classList.remove("active");
    });
    // Activate the clicked button
    btn.classList.add("active");
  };

  const handleButtonClick = (event, group) => {
    if (event.target.classList.contains("btn")) {
      activateButton(event.target, group);
    }

    // Store the active button in each group in session storage
    sessionStorage.setItem(
      `${network}-content-filter`,
      `#${contentGroup.querySelector(".active").id}`,
    );
    sessionStorage.setItem(
      `${network}-strategy-filter`,
      `#${strategyGroup.querySelector(".active").id}`,
    );
    sessionStorage.setItem(
      `${network}-audit-result-filter`,
      `#${auditResultGroup.querySelector(".active").id}`,
    );

    // Get the active button's filter string in each group
    const selectedContent = contentGroup
      .querySelector(".active")
      .getAttribute("filter");
    const selectedStrategy = strategyGroup
      .querySelector(".active")
      .getAttribute("filter");
    const selectedSuccess = auditResultGroup
      .querySelector(".active")
      .getAttribute("filter");

    updateDashboard(
      network,
      selectedStrategy,
      selectedContent,
      selectedSuccess,
    );
  };

  // Check whether the browser's session storage contains a filter for the given group, otherwise use default
  const setInitialButton = (filter, defaultButton, group) => {
    if (sessionStorage.getItem(filter) !== null) {
      activateButton(
        document.querySelector(`${sessionStorage.getItem(filter)}`),
        group,
      );
    } else {
      activateButton(document.querySelector(defaultButton), group);
    }
  };

  // Attach event listeners to each button group
  contentGroup.addEventListener("click", (event) =>
    handleButtonClick(event, contentGroup),
  );
  strategyGroup.addEventListener("click", (event) =>
    handleButtonClick(event, strategyGroup),
  );
  auditResultGroup.addEventListener("click", (event) =>
    handleButtonClick(event, auditResultGroup),
  );

  setInitialButton(
    `${network}-content-filter`,
    "#all-content-button",
    contentGroup,
  );
  setInitialButton(
    `${network}-strategy-filter`,
    "#all-strategy-button",
    strategyGroup,
  );
  setInitialButton(
    `${network}-audit-result-filter`,
    "#all-audit-result-button",
    auditResultGroup,
  );

  const selectedContent = contentGroup
    .querySelector(".active")
    .getAttribute("filter");
  const selectedStrategy = strategyGroup
    .querySelector(".active")
    .getAttribute("filter");
  const selectedSuccess = auditResultGroup
    .querySelector(".active")
    .getAttribute("filter");

  updateDashboard(network, selectedStrategy, selectedContent, selectedSuccess);
};
