// Finite fixture data and scripted Host responses, shared by Node and Chromium.
// No glob matching, evaluation, URI normalization, or result formatting lives here.
export const codeUnits = text => Array.from({length: text.length}, (_, i) => text.charCodeAt(i));
export function unhex(text) {
  if (text.length % 4 || !/^[0-9a-f]*$/i.test(text)) throw Error('invalid UTF16 fixture hex');
  let result = '';
  for (let i = 0; i < text.length; i += 4) result += String.fromCharCode(parseInt(text.slice(i, i + 4), 16));
  return result;
}
function table(files, name, width) {
  const text = files[name];
  if (typeof text !== 'string' || /[^\x00-\x7f]/.test(text)) throw Error(`missing/non-ASCII fixture ${name}`);
  if (!text) return [];
  if (!text.endsWith('\n')) throw Error(`unterminated fixture ${name}`);
  return text.slice(0, -1).split('\n').map(line => {
    const row = line.split('\t');
    if (row.length !== width) throw Error(`invalid fixture width ${name}`);
    return row;
  });
}
function flag(value) {
  if (value !== 'true' && value !== 'false') throw Error('invalid fixture boolean');
  return value === 'true';
}
export function parseGlobFixture(files) {
  const config = new Map();
  for (const [key, value] of table(files, 'config.tsv', 2)) {
    if (config.has(key)) throw Error('duplicate fixture config');
    config.set(key, unhex(value));
  }
  const listings = new Map();
  for (const [key, name, directory] of table(files, 'listings.tsv', 3)) {
    const uri = unhex(key);
    if (!listings.has(uri)) listings.set(uri, []);
    listings.get(uri).push({name: new Uint16Array(codeUnits(unhex(name))), isDirectory: flag(directory)});
  }
  for (const [key] of table(files, 'empty-lists.tsv', 1)) if (!listings.has(unhex(key))) listings.set(unhex(key), []);
  const pairs = name => new Map(table(files, name, 2).map(row => row.map(unhex)));
  const rules = table(files, 'rules.tsv', 5).map(([operation, uri, attempt, kind, value]) => {
    if (!/^[1-9][0-9]*$/.test(attempt) || !Number.isSafeInteger(Number(attempt))) throw Error('invalid fixture attempt');
    return {operation, uri: unhex(uri), attempt: Number(attempt), kind, value: unhex(value)};
  });
  return {config, listings, modules: pairs('modules.tsv'), resources: pairs('resources.tsv'),
    present: new Set(table(files, 'present.tsv', 1).map(([uri]) => unhex(uri))), rules,
    owners: table(files, 'owner-config.tsv', 4).map(row => row.map(unhex)),
    queries: table(files, 'queries.tsv', 2).map(([id, expression]) => ({id, expression: unhex(expression)})),
    source: files['module.pkl']};
}

// These are fixture decisions, not synthesized Java SPI callbacks. The driver
// records the actual Rust typed request/completion separately, including all IDs.
export class GlobFixtureProvider {
  constructor(fixture) { this.fixture = fixture; this.attempts = new Map(); this.decisions = []; this.phase = 'bootstrap'; }
  decision(operation, owner, values) {
    this.decisions.push({index: this.decisions.length, phase: this.phase, operation, owner, values: values.map(codeUnits)});
  }
  setting(owner, key) {
    let value = this.fixture.config.get(key);
    for (const [prefix, suffix, name, configured] of this.fixture.owners)
      if (owner.startsWith(prefix) && owner.endsWith(suffix) && key === name) value = configured;
    return value;
  }
  boolean(owner, key) { return flag(this.setting(owner, key)); }
  rule(operation, uri) {
    const config = this.fixture.config;
    if (['resolve', 'load-source', 'read', 'fragment-paths', 'globbable', 'hierarchical'].includes(operation)
        && [config.get('sourceUri'), config.get('resolvedPrimary')].includes(uri)) return null;
    const key = `${operation}\0${uri}`;
    const attempt = (this.attempts.get(key) ?? 0) + 1; this.attempts.set(key, attempt);
    let selected = null;
    for (const rule of this.fixture.rules) {
      const match = rule.uri === '*' || rule.uri === uri || (rule.uri.endsWith(':*') && uri.startsWith(rule.uri.slice(0, -1)));
      if (rule.operation === operation && match && rule.attempt <= attempt
          && (!selected || rule.attempt > selected.attempt || (rule.uri !== '*' && selected.uri === '*'))) selected = rule;
    }
    if (selected) this.decision('fixture.rule', operation, [uri, String(attempt), selected.kind, selected.value]);
    return selected;
  }
  resourceReader(scheme) {
    let selected = null;
    let index = 0;
    for (const candidate of this.fixture.config.get('resourceSchemes').split(',')) {
      if (!candidate) continue;
      if (candidate === scheme) selected = {owner: `resource:${candidate}:${index}`, scheme: candidate, selectedUri: `${candidate}:configured`};
      index++;
    }
    return selected;
  }
  moduleReader(uri) {
    if (!uri.includes(':')) return null;
    const scheme = uri.slice(0, uri.indexOf(':'));
    const config = this.fixture.config;
    const groups = (config.get('moduleProviderSchemes') ?? config.get('moduleSchemes')).split(';');
    const index = groups.findIndex(group => group.split(',').includes(scheme));
    return index < 0 ? null : {owner: `module:${uri}@${index}`, selectedUri: uri, providerIndex: index};
  }
  elementsAfterAdmission(reader, uri, rule) {
    const base = this.boolean(reader.owner, 'hierarchical') ? uri : '*';
    const key = rule?.kind === 'elements' ? `script:${rule.value}` : (this.setting(reader.owner, 'listingPrefix') ?? '') + base;
    const elements = this.fixture.listings.get(key);
    if (!elements) throw Error(`unconfigured listing ${key}`);
    return {elements: elements.map(element => ({name: new Uint16Array(element.name), isDirectory: element.isDirectory}))};
  }
  exists(uri) { return {rule: this.rule('has-element', uri), exists: this.fixture.present.has(uri)}; }
  resource(uri) {
    const rule = this.rule('read', uri);
    if (rule) return {rule};
    if (!this.fixture.resources.has(uri)) throw Error(`unconfigured resource ${uri}`);
    return {value: this.fixture.resources.get(uri)};
  }
  moduleSource(uri) {
    const rule = this.rule('load-source', uri);
    if (rule) return {rule};
    if (!this.fixture.modules.has(uri)) throw Error(`unconfigured module source ${uri}`);
    return {source: this.fixture.modules.get(uri)};
  }
}
