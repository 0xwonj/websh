const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
const source = readFileSync('tools/migrate-browser-state.js', 'utf8');
function storage(entries) {
  const values = new Map(Object.entries(entries));
  return {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: key => values.delete(key),
  };
}
test('migration preserves newer preferences, canonicalizes aliases, and is repeatable', () => {
  const localStorage = storage({ 'reader.TEXT_SCALE': 'xl', 'websh.reader.scale': 'small', 'websh.dino_game.high_score.v1': '123', 'user.THEME': 'wave', unrelated: 'keep' });
  const sessionStorage = storage({ 'websh.gh_token': 'discard-without-reading' });
  const context = { localStorage, sessionStorage };
  runInNewContext(source, context);
  runInNewContext(source, context);
  assert.equal(localStorage.getItem('websh.reader.scale'), 'small');
  assert.equal(localStorage.getItem('websh.dino.score'), '123');
  assert.equal(localStorage.getItem('user.THEME'), 'kanagawa-wave');
  assert.equal(localStorage.getItem('reader.TEXT_SCALE'), null);
  assert.equal(localStorage.getItem('unrelated'), 'keep');
  assert.equal(sessionStorage.getItem('websh.gh_token'), null);
});
test('a failed write preserves the old preference', () => {
  const localStorage = storage({ 'reader.TEXT_SCALE': 'large' });
  localStorage.setItem = () => { throw new Error('quota'); };
  assert.throws(() => runInNewContext(source, { localStorage, sessionStorage: storage({}) }), /quota/);
  assert.equal(localStorage.getItem('reader.TEXT_SCALE'), 'large');
});
