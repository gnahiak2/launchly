type Plan = { framework:string; language:string; package_manager:string; build_command:string; start_command:string; port:number; confidence:string; rationale:string[]; repository_url:string };
type Deployment = { id:string; state:string; message:string; plan?:Plan; host_port?:number };
const form = document.querySelector<HTMLFormElement>('#deploy-form')!;
const repository = document.querySelector<HTMLInputElement>('#repository')!;
const domain = document.querySelector<HTMLInputElement>('#domain')!;
const inspectButton = document.querySelector<HTMLButtonElement>('#inspect')!;
const deployButton = document.querySelector<HTMLButtonElement>('#deploy')!;
const message = document.querySelector<HTMLElement>('#message')!;
const planSection = document.querySelector<HTMLElement>('#plan')!;
const details = document.querySelector<HTMLElement>('#plan-details')!;
const rationale = document.querySelector<HTMLUListElement>('#rationale')!;
const statusSection = document.querySelector<HTMLElement>('#status')!;
let currentPlan: Plan | undefined;
const showMessage = (text:string, good=false) => { message.textContent=text; message.hidden=false; message.className=good?'message ok':'message'; };
const request = async <T>(url:string, options?:RequestInit):Promise<T> => { const response=await fetch(url, options); const data=await response.json(); if(!response.ok) throw new Error(data.error || 'Request failed'); return data; };
function renderPlan(value:Plan){ currentPlan=value; planSection.hidden=false; deployButton.disabled=false; details.innerHTML=''; const fields:[string,string|number][]=[['Repository',value.repository_url],['Framework',value.framework],['Language',value.language],['Build',value.build_command],['Server',value.start_command],['Port',value.port],['Confidence',value.confidence]]; for(const [name,text] of fields){const dt=document.createElement('dt');dt.textContent=name;const dd=document.createElement('dd');dd.textContent=String(text);details.append(dt,dd)} rationale.replaceChildren(...value.rationale.map(item=>{const li=document.createElement('li');li.textContent=item;return li;})); }
inspectButton.onclick=async()=>{ try{inspectButton.disabled=true;showMessage('Cloning and inspecting the repository…');renderPlan(await request<Plan>('/api/plans',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({repository_url:repository.value})}));showMessage('Repository is ready to deploy.',true);}catch(error){showMessage((error as Error).message)}finally{inspectButton.disabled=false;} };
form.onsubmit=async event=>{event.preventDefault();if(!currentPlan){return}try{deployButton.disabled=true;const deployment=await request<Deployment>('/api/deployments',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({repository_url:repository.value,domain:domain.value||null})});statusSection.hidden=false;poll(deployment.id);}catch(error){showMessage((error as Error).message);deployButton.disabled=false;}};
async function poll(id:string){try{const deployment=await request<Deployment>(`/api/deployments/${id}`);statusSection.hidden=false;document.querySelector('#state')!.textContent=deployment.state;document.querySelector('#status-message')!.textContent=deployment.message;if(deployment.state==='live'){showMessage('Website deployed successfully.',true);return}if(deployment.state==='failed'){showMessage(deployment.message);deployButton.disabled=false;return}setTimeout(()=>poll(id),1500);}catch(error){showMessage((error as Error).message);deployButton.disabled=false;}}
