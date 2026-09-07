const deployments = [
  { name: "launchly-web", repo: "thioj/launchly", branch: "main", status: "live", label: "Live", icon: "L", deployed: "12 minutes ago", duration: "1m 42s" },
  { name: "docs-site", repo: "thioj/docs", branch: "main", status: "live", label: "Live", icon: "D", iconClass: "orange", deployed: "Yesterday", duration: "48s" },
  { name: "api-service", repo: "thioj/api-service", branch: "develop", status: "building", label: "Building", icon: "A", iconClass: "blue", deployed: "Just now", duration: "—" },
];

const list = document.querySelector("#deployment-list");
const search = document.querySelector("#deployment-search");
const noResults = document.querySelector("#no-results");
const resultCount = document.querySelector("#result-count");
const toast = document.querySelector("#toast");
let toastTimer;

function deploymentRow(deployment) {
  const statusClass = deployment.status;
  return `<tr tabindex="0" data-deployment="${deployment.name}">
    <td><div class="app-cell"><span class="app-icon ${deployment.iconClass || ""}">${deployment.icon}</span>${deployment.name}</div></td>
    <td><span class="status ${statusClass}"><span class="status-dot"></span>${deployment.label}</span></td>
    <td class="repo-cell">${deployment.repo} <span class="faint">· ${deployment.branch}</span></td>
    <td>${deployment.deployed}</td><td>${deployment.duration}</td>
    <td><button class="row-menu" type="button" aria-label="Open ${deployment.name} actions">⋯</button></td>
  </tr>`;
}

function renderDeployments(items = deployments) {
  list.innerHTML = items.map(deploymentRow).join("");
  resultCount.textContent = items.length;
  noResults.classList.toggle("hidden", items.length > 0);
  list.parentElement.classList.toggle("hidden", items.length === 0);
  list.querySelectorAll("tr").forEach((row) => {
    row.addEventListener("click", (event) => {
      if (!event.target.closest("button")) showToast(`Opening ${row.dataset.deployment}`);
    });
    row.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        showToast(`Opening ${row.dataset.deployment}`);
      }
    });
  });
}

function showToast(message) {
  clearTimeout(toastTimer);
  toast.textContent = message;
  toast.classList.add("visible");
  toastTimer = setTimeout(() => toast.classList.remove("visible"), 2800);
}

function setPlanVisible(url) {
  const repoName = url.split("/").filter(Boolean).pop()?.replace(/\.git$/, "") || "your-app";
  document.querySelector("#plan-stack").textContent = repoName.toLowerCase().includes("python") ? "Python" : "Node.js";
  document.querySelector("#plan-reason-text").textContent = `Detected package.json and package-lock.json in ${repoName}.`;
  document.querySelector("#plan-empty").classList.add("hidden");
  document.querySelector("#plan-content").classList.remove("hidden");
  document.querySelector("#plan-heading").scrollIntoView({ behavior: "smooth", block: "center" });
}

const form = document.querySelector("#deploy-form");
const repoInput = document.querySelector("#repo-url");
const formMessage = document.querySelector("#form-message");
form.addEventListener("submit", (event) => {
  event.preventDefault();
  formMessage.textContent = "";
  if (!repoInput.value.trim() || !repoInput.checkValidity()) {
    formMessage.textContent = "Enter a valid repository URL to continue.";
    repoInput.focus();
    return;
  }
  setPlanVisible(repoInput.value.trim());
  showToast("Repository inspected — review your deployment plan");
});

document.querySelector(".clear-input").addEventListener("click", () => {
  repoInput.value = "";
  repoInput.focus();
  formMessage.textContent = "";
});

document.querySelector("[data-scroll-to]").addEventListener("click", (event) => {
  document.getElementById(event.currentTarget.dataset.scrollTo).scrollIntoView({ behavior: "smooth", block: "center" });
  setTimeout(() => repoInput.focus(), 450);
});

document.querySelector("#review-plan").addEventListener("click", () => showToast("Full plan review will be available when the API is connected"));
search.addEventListener("input", () => {
  const query = search.value.trim().toLowerCase();
  renderDeployments(deployments.filter((deployment) => `${deployment.name} ${deployment.repo} ${deployment.branch}`.toLowerCase().includes(query)));
});
document.querySelector("#filter-button").addEventListener("click", () => showToast("Filters will be available when more deployments exist"));
document.querySelectorAll(".page-button:not(:disabled)").forEach((button) => button.addEventListener("click", () => showToast("Pagination will be connected to the deployment API")));

document.querySelectorAll(".nav-item").forEach((item) => item.addEventListener("click", () => {
  document.querySelectorAll(".nav-item").forEach((nav) => nav.classList.remove("active"));
  item.classList.add("active");
}));

renderDeployments();
