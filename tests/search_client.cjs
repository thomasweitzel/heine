// Run with `node tests/search_client.cjs` from the repository root.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

const script = fs.readFileSync('starter-site/kits/starter/assets/js/search.js', 'utf8');

class Element {
  constructor() {
    this.children = [];
    this.dataset = {};
    this.value = '';
    this.hidden = false;
    this.listeners = {};
    this.attributes = {};
  }

  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  setAttribute(name, value) { this.attributes[name] = value; }
  focus(options) { this.ownerDocument.activeElement = this; this.focusOptions = options; }
  addEventListener(name, listener) { this.listeners[name] = listener; }
  querySelector(selector) { return selector === 'input[name=q]' ? this.input : null; }
}

async function search(documents, query, inspect = false) {
  const form = new Element();
  const results = new Element();
  const status = new Element();
  const navigation = new Element();
  form.input = new Element();
  form.dataset = {
    searchUrl: '/index.json', loading: 'Loading', resultsOne: '__count__ result',
    resultsOther: '__count__ results', noResults: 'none', failure: 'error',
  };
  const elements = {
    '.search-form': form, '.search-results': results,
    '.search-status': status, '.search-pagination': navigation,
  };
  const document = {
    documentElement: { lang: 'en' },
    querySelector(selector) { return elements[selector]; },
    createElement() { const element = new Element(); element.ownerDocument = this; return element; },
  };
  const timers = new Map();
  const history = [];
  let nextTimer = 0;
  const window = {
    location: { search: '?q=' + encodeURIComponent(query), origin: 'https://example.com', pathname: '/search/' },
    history: { replaceState() { history.push('replace'); }, pushState() { history.push('push'); } },
    addEventListener() {},
    setTimeout(callback) { const id = ++nextTimer; timers.set(id, callback); return id; },
    clearTimeout(id) { timers.delete(id); },
  };
  const fetch = async () => ({ ok: true, json: async () => ({ format: 3, documents }) });
  vm.runInNewContext(script, { document, window, fetch, URL, URLSearchParams, Intl, console });
  await new Promise(resolve => setImmediate(resolve));
  assert.notEqual(status.textContent, 'error');
  if (inspect) return { document, form, navigation, timers, history };
  return results.children.map(item => {
    const link = item.children[0];
    return { title: link.children[0].children[0].textContent, excerpt: link.children[1]?.textContent, href: link.href };
  });
}

function document(title, description, summary, text, fragment = null) {
  return { title, description, summary, url: '/' + title, sections: text ? [{ text, fragment }] : [] };
}

async function main() {
  const documents = [
    document('Alpha', 'First note', 'First note', 'Garden body', 'garden'),
    document('Beta', 'Different words', 'Secret phrase', 'Other body'),
    document('Gamma', null, 'A fallback', ''),
    document('First', null, null, ''),
  ];
  let results = await search(documents, 'first');
  assert.equal(results[0].title, 'First');
  assert.equal(results[1].excerpt, 'First note');
  assert.equal(results[1].href, '/Alpha');
  results = await search(documents, 'secret');
  assert.equal(results[0].title, 'Beta');
  assert.equal(results[0].excerpt, 'Secret phrase');
  results = await search(documents, 'alpha note');
  assert.equal(results[0].title, 'Alpha');
  assert.equal(results[0].excerpt, 'First note');
  const mixed = [document('Alpha', 'About Alpha', 'Secret detail', 'Alpha body', 'body')];
  results = await search(mixed, 'alpha secret');
  assert.equal(results[0].excerpt, 'Secret detail');
  assert.equal(results[0].href, '/Alpha');
  const sections = [{ text: 'Alpha introduction', fragment: 'intro' },
    { text: 'A secret detail appears here', fragment: 'detail' }];
  results = await search([{ ...mixed[0], sections }], 'alpha secret');
  assert.equal(results[0].excerpt, 'A secret detail appears here');
  assert.equal(results[0].href, '/Alpha#detail');
  const repeated = [{ text: 'Alpha Alpha secret', fragment: 'first' },
    { text: 'Secret and secret again', fragment: 'second' }];
  results = await search([{ ...mixed[0], sections: repeated }], 'alpha secret');
  assert.equal(results[0].href, '/Alpha#second');
  results = await search(documents, 'garden note');
  assert.equal(results[0].excerpt, 'Garden body');
  assert.equal(results[0].href, '/Alpha#garden');
  results = await search(documents, 'gamma');
  assert.equal(results[0].excerpt, 'A fallback');
  const titleMatch = document('Making room for ideas', 'A short description.', null,
    'Come back to it later.', 'body');
  results = await search([titleMatch], 'for i');
  assert.equal(results[0].excerpt, 'A short description.');
  assert.equal(results[0].href, '/Making%20room%20for%20ideas');
  const introduction = document('Matching title', null, null, 'An introduction. ' + 'word '.repeat(60), 'intro');
  results = await search([introduction], 'matching');
  assert.ok(results[0].excerpt.startsWith('An introduction.'));
  assert.ok(results[0].excerpt.endsWith('…'));
  assert.ok(Array.from(results[0].excerpt).length <= 201);
  assert.equal(results[0].href, '/Matching%20title');
  results = await search([document('Bare title', null, null, '')], 'bare');
  assert.equal(results[0].excerpt, undefined);
  assert.equal((await search(documents, 'phrase missing')).length, 0);
  assert.equal((await search(documents, 'irst')).length, 0);
  results = await search([document('Overlap', null, null, 'xa-a-a-')], 'a-a-');
  assert.equal(results.length, 1);
  assert.equal(results[0].excerpt, 'xa-a-a-');
  const tagged = { ...document('Tagged', null, null, ''), taxonomies: { tags: ['exclusive'] } };
  assert.equal((await search([tagged], 'exclusive')).length, 0);

  const duplicates = [
    document('One', 'shared', null, ''),
    document('Two', 'shared', 'shared', ''),
  ];
  results = await search(duplicates, 'shared');
  assert.deepEqual(results.map(result => result.title), ['One', 'Two']);

  const pages = Array.from({ length: 11 }, (_, index) => document(`Page ${index}`, null, null, 'shared'));
  const ui = await search(pages, 'shared', true);
  const pageTwo = ui.navigation.children.find(node => node.textContent === 2);
  assert.ok(pageTwo, 'page two link');
  for (const modifier of ['ctrlKey', 'metaKey', 'shiftKey', 'altKey']) {
    let prevented = false;
    pageTwo.listeners.click({ button: 0, [modifier]: true, preventDefault() { prevented = true; } });
    assert.equal(prevented, false, modifier);
  }
  let prevented = false;
  pageTwo.listeners.click({ button: 1, preventDefault() { prevented = true; } });
  assert.equal(prevented, false, 'middle click');
  pageTwo.focus();
  pageTwo.listeners.click({ button: 0, preventDefault() { prevented = true; } });
  assert.equal(prevented, true, 'ordinary click');
  assert.deepEqual(ui.history, ['push']);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(ui.document.activeElement.attributes['aria-current'], 'page');
  assert.equal(ui.document.activeElement.attributes.tabindex, '-1');
  assert.equal(ui.document.activeElement.focusOptions.preventScroll, true);

  const moved = await search(pages, 'shared', true);
  const movedLink = moved.navigation.children.find(node => node.textContent === 2);
  movedLink.focus();
  movedLink.listeners.click({ button: 0, preventDefault() {} });
  moved.document.activeElement = moved.form.input;
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(moved.document.activeElement, moved.form.input, 'do not steal moved focus');

  ui.form.input.value = 'shared';
  ui.form.input.listeners.input();
  assert.equal(ui.timers.size, 1);
  ui.form.listeners.submit({ preventDefault() {} });
  assert.equal(ui.timers.size, 0, 'submit cancels pending input');
  console.log('Search client checks passed.');
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
