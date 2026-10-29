'use strict';
const menu = document.querySelector('#menu');
const sidebar = document.querySelector('#sidebar');
menu.addEventListener('click', () => {
  const open = menu.getAttribute('aria-expanded') !== 'true';
  menu.setAttribute('aria-expanded', String(open));
  sidebar.classList.toggle('is-open', open);
});
for (const link of sidebar.querySelectorAll('nav a')) {
  link.addEventListener('click', () => { menu.setAttribute('aria-expanded', 'false'); sidebar.classList.remove('is-open'); });
}
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && menu.getAttribute('aria-expanded') === 'true') {
    menu.click(); menu.focus();
  }
});
const navigation = [...sidebar.querySelectorAll('nav a')];
navigation[0].setAttribute('aria-current', 'location');
const observer = new IntersectionObserver(entries => {
  for (const entry of entries) if (entry.isIntersecting) {
    for (const link of navigation) {
      if (link.hash === '#' + entry.target.id) link.setAttribute('aria-current', 'location');
      else link.removeAttribute('aria-current');
    }
  }
}, { rootMargin: '-90px 0px -65% 0px', threshold: 0 });
document.querySelectorAll('main section').forEach(section => observer.observe(section));

const search = document.querySelector('#parameter-search');
search.disabled = false;
document.querySelector('#command').disabled = false;
const rows = [...document.querySelectorAll('#parameter-table tbody tr')];
search.addEventListener('input', () => {
  const query = search.value.trim().toLowerCase();
  let count = 0;
  for (const row of rows) { row.hidden = !row.textContent.toLowerCase().includes(query); if (!row.hidden) count++; }
  document.querySelector('#parameter-count').textContent = `${count} option${count === 1 ? '' : 's'}`;
  document.querySelector('#no-results').hidden = count !== 0;
});

let reference;
async function loadReference() {
  if (!reference) reference = fetch('cli-reference.json').then(response => {
    if (!response.ok) throw new Error('CLI reference unavailable');
    return response.json();
  }).catch(error => { reference = null; throw error; });
  return reference;
}
document.querySelector('#command').addEventListener('change', async event => {
  const command = event.target.value;
  const status = document.querySelector('#reference-status');
  status.textContent = 'Loading command reference…';
  try {
    const data = await loadReference();
    if (document.querySelector('#command').value !== command) return;
    if (!data.commands[command]) throw new Error('Command unavailable');
    document.querySelector('#command-help').textContent = data.commands[command];
    status.textContent = '';
  } catch {
    if (document.querySelector('#command').value === command) status.textContent = 'Could not load the reference. Select a command again to retry, or open the complete text reference below.';
  }
});

for (const pre of document.querySelectorAll('pre')) {
  const button = document.createElement('button');
  button.type = 'button'; button.className = 'copy'; button.textContent = 'Copy';
  button.setAttribute('aria-label', 'Copy code block');
  button.addEventListener('click', async () => {
    const code = pre.querySelector('code');
    try { await navigator.clipboard.writeText(code.textContent); button.textContent = 'Copied'; }
    catch {
      const range = document.createRange(); range.selectNodeContents(code);
      const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(range);
      button.textContent = 'Selected';
    }
    setTimeout(() => { button.textContent = 'Copy'; }, 1600);
  });
  pre.append(button);
}
