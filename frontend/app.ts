interface DeploymentPlan {
  framework: string;
  language: string;
  package_manager: string;
  build_command: string;
  start_command: string;
  port: number;
  confidence: string;
  rationale: string[];
}

interface DeploymentRecord {
  id: string;
  state: string;
  phase: string;
  message: string;
  plan: DeploymentPlan | null;
  image: string | null;
  container: string | null;
  host_port: number | null;
}

interface ErrorResponse { error: string }

const form = document.querySelector<HTMLFormElement>('#deploy-form')!;
const repository = document.querySelector<HTMLInputElement>('#repository-url')!;
const domain = document.querySelector<HTMLInputElement>('#domain')!;
const planButton = document.querySelector<HTMLButtonElement>('#plan-button')!;
const deployButton = document.querySelector<HTMLButtonElement>('#deploy-button')!;
const message = document.querySelector<HTMLElement>('#form-message')!;
const emptyPlan = document.querySelector<HTMLElement>('#empty-plan')!;
const planContent = document.querySelector<HTMLElement>('#plan-content')!;
const confidence = document.querySelector<HTMLElement>('#confidence')!;
const statusBadge = document.querySelector<HTMLElement>('#status-badge')!;
const statusContent = document.querySelector<HTMLElement>('#deployment-status')!;

function setText(id: string, value: string): void {
  document.querySelector<HTMLElement>(id)!.textContent = value;
}

function showMessage(text: string, error = false): void {
  message.textContent = text;
  message.classList.toggle('error', error);
}

async function request<T extends object>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  const data = await response.json() as T | ErrorResponse;
  if (!response.ok) throw new Error('error' in data ? data.error : 'Request failed');
  return data as T;
}

function renderPlan(plan: DeploymentPlan): void {
  emptyPlan.hidden = true;
  planContent.hidden = false;
  confidence.textContent = `Confidence: ${plan.confidence}`;
  setText('#plan-framework', plan.framework);
  setText('#plan-language', plan.language);
  setText('#plan-package-manager', plan.package_manager);
  setText('#plan-port', String(plan.port));
  setText('#plan-build', plan.build_command);
  setText('#plan-start', plan.start_command);
  document.querySelector('#plan-rationale')!.replaceChildren(...plan.rationale.map((reason) => {
    const item = document.createElement('li');
    item.textContent = reason;
    return item;
  }));
  // The URL-only preview can be low-confidence; the deployment job performs
  // authoritative repository inspection before building anything.
  deployButton.disabled = false;
}

async function inspect(): Promise<DeploymentPlan | null> {
  const repositoryUrl = repository.value.trim();
  if (!repositoryUrl) { showMessage('Enter a repository URL first.', true); repository.focus(); return null; }
  planButton.disabled = true;
  deployButton.disabled = true;
  showMessage('Inspecting repository…');
  try {
    const plan = await request<DeploymentPlan>('/api/plans', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ repository_url: repositoryUrl }),
    });
    renderPlan(plan);
    showMessage('Plan ready. Review it before deploying.');
    return plan;
  } catch (error) {
    showMessage(error instanceof Error ? error.message : 'Unable to inspect repository.', true);
    return null;
  } finally { planButton.disabled = false; }
}

function renderStatus(record: DeploymentRecord): void {
  statusBadge.textContent = record.state;
  statusBadge.dataset.state = record.state;
  statusContent.replaceChildren();
  const summary = document.createElement('p');
  summary.textContent = `${record.phase}: ${record.message}`;
  statusContent.append(summary);
  if (record.plan) renderPlan(record.plan);
}

async function pollDeployment(id: string): Promise<void> {
  const terminal = new Set(['live', 'failed']);
  for (;;) {
    const record = await request<DeploymentRecord>(`/api/deployments/${encodeURIComponent(id)}`);
    renderStatus(record);
    if (terminal.has(record.state)) {
      deployButton.disabled = record.state !== 'live';
      return;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 1500));
  }
}

planButton.addEventListener('click', () => { void inspect(); });
form.addEventListener('submit', async (event) => {
  event.preventDefault();
  const plan = await inspect();
  if (!plan) return;
  deployButton.disabled = true;
  showMessage('Queueing deployment…');
  try {
    const record = await request<DeploymentRecord>('/api/deployments', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ repository_url: repository.value.trim(), domain: domain.value.trim() || null }),
    });
    renderStatus(record);
    await pollDeployment(record.id);
    showMessage('Deployment finished.');
  } catch (error) {
    showMessage(error instanceof Error ? error.message : 'Deployment failed.', true);
    deployButton.disabled = false;
  }
});
