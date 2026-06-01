/* global acquireVsCodeApi, GLANCE_CONFIG */
'use strict';

// ── Bootstrap ─────────────────────────────────────────────────────────────────

const vscode = acquireVsCodeApi();
const CFG = window.GLANCE_CONFIG;

// ── State machine ─────────────────────────────────────────────────────────────
// Single source of truth — no scattered globals.

const state = {
  // Pagination
  offset: 0,
  // Toggles
  usePretty: false,
  useRegex: false,
  // Search
  isSearching: false,
  allMatches: [],
  matchIdx: -1,
  focusedLine: -1,
  savedTotal: 0,
  savedTruncated: false,
  searchGen: 0,
  // Expected offset for current read request — stale responses are ignored
  expectedOffset: -1,
};

// Restore persisted state from VS Code (survives panel hide/show cycle)
const saved = vscode.getState();
if (saved) {
  state.offset    = saved.offset    ?? 0;
  state.usePretty = saved.usePretty ?? false;
  state.useRegex  = saved.useRegex  ?? false;
}

function persist() {
  vscode.setState({ offset: state.offset, usePretty: state.usePretty, useRegex: state.useRegex });
}

const PAGE_RAW    = 200;
const PAGE_PRETTY = 10;
const pageSize    = () => state.usePretty ? PAGE_PRETTY : PAGE_RAW;

// ── DOM refs ──────────────────────────────────────────────────────────────────

const $ = id => document.getElementById(id);

const els = {
  info:        $('info'),
  search:      $('search'),
  lines:       $('lines'),
  pageInfo:    $('page-info'),
  matchCount:  $('match-count'),
  matchInfo:   $('match-nav-info'),
  toast:       $('toast'),
  btnPrev:     $('btn-prev'),
  btnNext:     $('btn-next'),
  btnRegex:    $('btn-regex'),
  btnPretty:   $('btn-pretty'),
  btnMPrev:    $('btn-match-prev'),
  btnMNext:    $('btn-match-next'),
  gotoInput:   $('goto-input'),
};

// ── Init UI ───────────────────────────────────────────────────────────────────

els.info.textContent =
  CFG.totalLines.toLocaleString() + ' lines • ' + CFG.fileSizeMb + ' MB • ' + CFG.format.toUpperCase();

if (CFG.isJsonl) { els.btnPretty.classList.add('visible'); }
if (state.usePretty) { els.btnPretty.classList.add('active'); }
if (state.useRegex)  { els.btnRegex.classList.add('active'); els.search.classList.add('regex-active'); }

// ── Clipboard / Toast ─────────────────────────────────────────────────────────

let currentLines = []; // for click-to-copy

function copyText(text) {
  navigator.clipboard.writeText(text).then(() => showToast('Copied!'));
}

function showToast(msg) {
  els.toast.textContent = msg;
  els.toast.classList.add('show');
  setTimeout(() => els.toast.classList.remove('show'), 1500);
}

// ── Request helpers ───────────────────────────────────────────────────────────

function sendRead(offset, limit, pretty) {
  state.expectedOffset = offset;
  showLoading();
  vscode.postMessage({ type: 'read', offset, limit, pretty });
}

function showLoading() {
  els.lines.innerHTML = '<div id="status">Loading…</div>';
}

function showError(msg) {
  els.lines.innerHTML = '<div id="error-msg">Error: ' + esc(msg) + '</div>';
}

// ── Page navigation ───────────────────────────────────────────────────────────

function loadPage(newOffset) {
  const ps = pageSize();
  state.offset      = Math.max(0, Math.min(newOffset, Math.max(0, CFG.totalLines - ps)));
  state.isSearching = false;
  state.allMatches  = [];
  state.matchIdx    = -1;
  state.focusedLine = -1;
  state.savedTotal  = 0;
  state.savedTruncated = false;
  els.search.value  = '';
  els.search.classList.toggle('regex-active', state.useRegex);
  els.matchCount.textContent  = '';
  els.matchInfo.textContent   = '';
  els.btnMPrev.disabled = true;
  els.btnMNext.disabled = true;
  persist();
  sendRead(state.offset, ps, state.usePretty);
  updatePageButtons();
}

function prevPage() { loadPage(state.offset - pageSize()); }
function nextPage() { loadPage(state.offset + pageSize()); }

function updatePageButtons() {
  const ps = pageSize();
  els.btnPrev.disabled = state.offset <= 0;
  els.btnNext.disabled = state.offset + ps >= CFG.totalLines;
}

// ── Match navigation ──────────────────────────────────────────────────────────

function prevMatch() {
  if (state.matchIdx > 0) {
    jumpToMatch(state.matchIdx - 1);
  } else if (state.matchIdx === 0) {
    state.matchIdx    = -1;
    state.focusedLine = -1;
    renderSearch(state.allMatches, state.savedTotal, state.savedTruncated);
  }
}

function nextMatch() {
  if (state.matchIdx === -1 && state.allMatches.length > 0) {
    jumpToMatch(0);
  } else if (state.matchIdx >= 0 && state.matchIdx < state.allMatches.length - 1) {
    jumpToMatch(state.matchIdx + 1);
  }
}

function updateMatchButtons() {
  const hasNext = state.matchIdx === -1
    ? state.allMatches.length > 0
    : state.matchIdx < state.allMatches.length - 1;
  els.btnMPrev.disabled = state.matchIdx < 0;
  els.btnMNext.disabled = !hasNext;
  if (state.matchIdx >= 0) {
    const lineNum = state.allMatches[state.matchIdx].line_number + 1; // 1-indexed
    els.matchInfo.textContent =
      (state.matchIdx + 1) + ' / ' + state.allMatches.length + ' — Line ' + lineNum.toLocaleString();
  } else if (state.allMatches.length > 0) {
    els.matchInfo.textContent = state.allMatches.length + ' matches';
  } else {
    els.matchInfo.textContent = '';
  }
}

function jumpToMatch(idx) {
  if (idx < 0 || idx >= state.allMatches.length) { return; }
  state.matchIdx    = idx;
  state.focusedLine = state.allMatches[idx].line_number;
  const ps  = pageSize();
  const off = Math.max(0, state.focusedLine - Math.floor(ps / 2));
  updateMatchButtons();
  sendRead(off, ps, state.usePretty);
}

// ── Toggles ───────────────────────────────────────────────────────────────────

function toggleRegex() {
  state.useRegex = !state.useRegex;
  els.btnRegex.classList.toggle('active', state.useRegex);
  els.search.classList.toggle('regex-active', state.useRegex);
  els.search.placeholder = state.useRegex ? 'Regex pattern…' : 'Search in file…';
  persist();
  const q = els.search.value.trim();
  if (q) { triggerSearch(q); }
}

function togglePretty() {
  state.usePretty = !state.usePretty;
  els.btnPretty.classList.toggle('active', state.usePretty);
  persist();
  if (state.isSearching && state.matchIdx === -1 && state.allMatches.length > 0) {
    renderSearch(state.allMatches, state.savedTotal, state.savedTruncated);
  } else if (state.isSearching && state.matchIdx >= 0) {
    jumpToMatch(state.matchIdx);
  } else {
    loadPage(state.offset);
  }
}

// ── Search ────────────────────────────────────────────────────────────────────

let searchTimer = null;

function triggerSearch(q) {
  state.isSearching = true;
  state.allMatches  = [];
  state.matchIdx    = -1;
  state.focusedLine = -1;
  state.searchGen++;
  const gen = state.searchGen;
  els.matchCount.textContent = 'searching…';
  els.btnMPrev.disabled = true;
  els.btnMNext.disabled = true;
  updatePageButtons();
  vscode.postMessage({ type: 'search', query: q, useRegex: state.useRegex });
  vscode.postMessage({ type: 'count',  query: q, useRegex: state.useRegex, gen });
}

// ── Message handler ───────────────────────────────────────────────────────────

window.addEventListener('message', function(e) {
  const msg = e.data;

  if (msg.type === 'lines') {
    renderLines(msg.lines, msg.offset, msg.total_lines);
  } else if (msg.type === 'search_results') {
    renderSearch(msg.results, msg.total_found, msg.truncated);
  } else if (msg.type === 'count_result') {
    if (msg.gen === undefined || msg.gen === state.searchGen) {
      els.matchCount.textContent = msg.count.toLocaleString() + ' total';
    }
  } else if (msg.type === 'error') {
    showError(msg.message);
  }
});

// ── Renderers ─────────────────────────────────────────────────────────────────

function renderLines(lines, off, total) {
  // Ignore stale responses — user navigated to a different location before this arrived
  if (state.expectedOffset >= 0 && off !== state.expectedOffset) { return; }
  currentLines = lines;
  // Sync offset from server response (ensures goto/jumpToMatch keep state correct)
  if (!state.isSearching) { state.offset = off; }

  const el = els.lines;
  if (!lines.length) { el.innerHTML = '<div id="status">No lines found.</div>'; return; }

  if (CFG.isCsv && lines[0].fields) {
    el.innerHTML = renderCsvTable(lines);
  } else if (state.usePretty && CFG.isJsonl) {
    el.innerHTML = lines.map(function(l, idx) {
      const isFocused = state.focusedLine >= 0 && l.number === state.focusedLine;
      const fm = isFocused && state.matchIdx >= 0 ? state.allMatches[state.matchIdx] : null;
      return renderJsonCard(l, idx, fm, isFocused);
    }).join('');
  } else {
    el.innerHTML = lines.map(function(l, idx) {
      const isFocused = state.focusedLine >= 0 && l.number === state.focusedLine;
      const fm = isFocused && state.matchIdx >= 0 ? state.allMatches[state.matchIdx] : null;
      const content = fm ? highlightInContent(l.content, fm) : esc(l.content);
      return '<div class="line' + (isFocused ? ' line-focused' : '') + '">' +
        '<span class="line-num" data-idx="' + idx + '">' + (l.number + 1) + '</span>' +
        '<span class="line-content">' + content + '</span>' +
        '</div>';
    }).join('');
  }

  el.scrollTop = 0;

  // Defer scroll until after browser reflow.
  // Scroll to the .match span (highlighted text) if available,
  // else fall back to the focused line/card.
  // Use getBoundingClientRect() — reliable regardless of offsetParent hierarchy.
  if (state.focusedLine >= 0) {
    requestAnimationFrame(function() {
      const matchSpan = el.querySelector('.line-focused .match, .json-card.line-focused .match');
      const target    = matchSpan || el.querySelector('.line-focused, .json-card.line-focused');
      if (!target) { return; }
      const containerRect = el.getBoundingClientRect();
      const targetRect    = target.getBoundingClientRect();

      // Vertical scroll — center the target in the viewport
      const relativeTop = targetRect.top - containerRect.top + el.scrollTop;
      el.scrollTop = relativeTop - Math.floor((el.clientHeight - target.offsetHeight) / 2);

      // Horizontal scroll — only when scrolling to the match span (not the whole line/card).
      // In raw mode, match text may be far to the right in a long JSON line.
      if (matchSpan) {
        const relativeLeft = targetRect.left - containerRect.left + el.scrollLeft;
        el.scrollLeft = relativeLeft - Math.floor((el.clientWidth - target.offsetWidth) / 2);
      } else {
        el.scrollLeft = 0; // reset to left when showing card/line (no specific match position)
      }
    });
  }

  const end = Math.min(off + lines.length, total);
  els.pageInfo.textContent =
    'Lines ' + (off + 1).toLocaleString() + '–' + end.toLocaleString() + ' of ' + total.toLocaleString();
  updatePageButtons();
}

function renderSearch(results, totalFound, truncated) {
  state.allMatches     = results;
  state.savedTotal     = totalFound;
  state.savedTruncated = truncated;
  state.matchIdx       = -1;
  state.focusedLine    = -1;
  currentLines = results.map(function(r) { return { number: r.line_number, content: r.content }; });

  const el = els.lines;
  if (!results.length) {
    el.innerHTML = '<div id="status">No matches found.</div>';
    els.pageInfo.textContent = '0 matches';
    updateMatchButtons();
    return;
  }

  el.scrollTop = 0;

  if (state.usePretty && CFG.isJsonl) {
    el.innerHTML = results.map(renderSearchCard).join('');
  } else {
    el.innerHTML = results.map(function(r, idx) {
      return '<div class="line search-result">' +
        '<span class="line-num" data-idx="' + idx + '" title="Click to view in context">' + (r.line_number + 1) + '</span>' +
        '<span class="line-content">' + escHighlight(r.content, r.match_start, r.match_end) + '</span>' +
        '</div>';
    }).join('');
  }

  const suffix = truncated ? ' (first ' + results.length + ' shown)' : '';
  els.pageInfo.textContent = totalFound.toLocaleString() + ' matches' + suffix;
  updateMatchButtons();
}

function renderJsonCard(l, idx, focusedMatch, isFocused) {
  const isPretty = l.content.includes('\n');
  const lineNum  = 'data-line-num="' + l.number + '"';
  // line-focused enables scroll-into-view and visual indicator
  const focusedClass = isFocused ? ' line-focused' : '';

  if (isPretty) {
    let body;
    if (focusedMatch) {
      body = highlightInContent(l.content, focusedMatch);
    } else {
      body = esc(l.content);
    }
    return '<div class="json-card' + focusedClass + '" ' + lineNum + '>' +
      '<div class="json-card-header" data-idx="' + idx + '" title="Click to copy">' +
      '<span>Line ' + (l.number + 1) + '</span><span>⎘</span>' +
      '</div><div class="json-card-body">' + body + '</div></div>';
  }

  const PREVIEW = 150;
  const cardId  = 'raw-' + l.number;
  const preview = l.content.length > PREVIEW
    ? esc(l.content.slice(0, PREVIEW)) + '<span class="raw-ellipsis">… (' + l.content.length.toLocaleString() + ' chars)</span>'
    : esc(l.content);

  return '<div class="json-card raw' + focusedClass + '" ' + lineNum + '>' +
    '<div class="json-card-header" data-card-id="' + cardId + '">' +
    '<span>⚠ Line ' + (l.number + 1) + ' — not valid JSON</span>' +
    '<span id="' + cardId + '-arrow">▶ Show</span>' +
    '</div>' +
    '<div class="json-card-body raw-preview" id="' + cardId + '-preview">' + preview + '</div>' +
    '<div class="json-card-body raw-full" id="' + cardId + '-full" style="display:none">' +
    esc(l.content) +
    '<br><button class="raw-copy-btn" data-idx="' + idx + '">Copy</button>' +
    '</div></div>';
}

function renderSearchCard(r, idx) {
  let displayContent = r.content;
  let hlStart = r.match_start;
  let hlEnd   = r.match_end;

  if (r.content.trim().startsWith('{')) {
    try {
      const pretty = JSON.stringify(JSON.parse(r.content), null, 2);
      const matchText = r.content.slice(r.match_start, r.match_end);
      const pos = findMatchInPretty(pretty, matchText, r.match_start, r.content.length);
      displayContent = pretty;
      hlStart = pos >= 0 ? pos : -1;
      hlEnd   = pos >= 0 ? pos + matchText.length : -1;
    } catch (_) { /* keep raw */ }
  }

  const body = (hlStart >= 0 && hlEnd > hlStart)
    ? escHighlight(displayContent, hlStart, hlEnd)
    : esc(displayContent);

  return '<div class="json-card">' +
    '<div class="json-card-header search-nav" data-idx="' + idx + '" title="Click to view in context">' +
    '<span>Line ' + (r.line_number + 1) + '</span><span>→ view</span>' +
    '</div><div class="json-card-body">' + body + '</div></div>';
}

function renderCsvTable(lines) {
  const numCols = lines[0].fields.length;
  let headerHtml = '<tr><th class="row-num">#</th>';
  for (let i = 0; i < numCols; i++) { headerHtml += '<th>Col ' + (i + 1) + '</th>'; }
  headerHtml += '</tr>';
  const rowsHtml = lines.map(function(l, idx) {
    const fields = l.fields || [];
    let row = '<tr><td class="row-num" data-idx="' + idx + '">' + (l.number + 1) + '</td>';
    for (let i = 0; i < numCols; i++) {
      row += '<td title="' + esc(fields[i] || '') + '">' + esc(fields[i] || '') + '</td>';
    }
    return row + '</tr>';
  }).join('');
  return '<table id="csv-table"><thead>' + headerHtml + '</thead><tbody>' + rowsHtml + '</tbody></table>';
}

// ── Event delegation ──────────────────────────────────────────────────────────

els.lines.addEventListener('click', function(e) {
  const target = e.target;

  const lineNum = target.closest('.line-num, .row-num');
  if (lineNum) {
    const idx = parseInt(lineNum.dataset.idx, 10);
    if (!isNaN(idx) && currentLines[idx]) {
      if (state.isSearching && state.matchIdx === -1 && state.allMatches.length > 0) {
        jumpToMatch(idx);
      } else {
        copyText(currentLines[idx].content);
      }
    }
    return;
  }

  const cardHeader = target.closest('.json-card-header');
  if (cardHeader) {
    if (cardHeader.classList.contains('search-nav')) {
      const idx = parseInt(cardHeader.dataset.idx, 10);
      if (!isNaN(idx)) { jumpToMatch(idx); }
    } else if (cardHeader.dataset.cardId) {
      toggleRaw(cardHeader.dataset.cardId);
    } else {
      const idx = parseInt(cardHeader.dataset.idx, 10);
      if (!isNaN(idx) && currentLines[idx]) { copyText(currentLines[idx].content); }
    }
    return;
  }

  const copyBtn = target.closest('.raw-copy-btn');
  if (copyBtn) {
    const idx = parseInt(copyBtn.dataset.idx, 10);
    if (!isNaN(idx) && currentLines[idx]) { copyText(currentLines[idx].content); }
  }
});

function toggleRaw(cardId) {
  const preview = $( cardId + '-preview');
  const full    = $(cardId + '-full');
  const arrow   = $(cardId + '-arrow');
  if (!preview || !full || !arrow) { return; }
  const isOpen  = full.style.display !== 'none';
  preview.style.display = isOpen ? '' : 'none';
  full.style.display    = isOpen ? 'none' : '';
  arrow.textContent     = isOpen ? '▶ Show' : '▼ Hide';
}

// ── Button listeners ──────────────────────────────────────────────────────────

els.btnRegex.addEventListener('click', toggleRegex);
els.btnPretty.addEventListener('click', togglePretty);
els.btnPrev.addEventListener('click', prevPage);
els.btnNext.addEventListener('click', nextPage);
els.btnMPrev.addEventListener('click', prevMatch);
els.btnMNext.addEventListener('click', nextMatch);

els.search.addEventListener('input', function() {
  clearTimeout(searchTimer);
  const q = this.value.trim();
  if (!q) { loadPage(state.offset); return; }
  searchTimer = setTimeout(function() {
    const q2 = els.search.value.trim();
    if (q2) { triggerSearch(q2); }
  }, 300);
});

els.search.addEventListener('keydown', function(e) {
  if (e.key === 'Enter') { e.preventDefault(); if (e.shiftKey) { prevMatch(); } else { nextMatch(); } }
});

els.gotoInput.addEventListener('keydown', function(e) {
  if (e.key !== 'Enter') { return; }
  const n = parseInt(this.value, 10);
  if (isNaN(n) || n < 1 || n > CFG.totalLines) {
    this.classList.add('goto-error');
    setTimeout(function() { els.gotoInput.classList.remove('goto-error'); }, 800);
    return;
  }
  this.classList.remove('goto-error');
  this.value = '';
  loadPage(n - 1);
});

document.addEventListener('keydown', function(e) {
  if (e.ctrlKey && e.key === 'g') { e.preventDefault(); els.gotoInput.focus(); }
  if (e.ctrlKey && e.key === 'f') { e.preventDefault(); els.search.focus(); }
  if (e.ctrlKey && e.key === 'r') { e.preventDefault(); toggleRegex(); }
  if (e.key === 'Escape') {
    if (state.matchIdx >= 0) {
      state.matchIdx = -1; state.focusedLine = -1;
      renderSearch(state.allMatches, state.savedTotal, state.savedTruncated);
    } else if (els.search.value) {
      els.search.value = ''; loadPage(state.offset);
    }
  }
  if (e.altKey && e.key === 'ArrowLeft')  { if (!els.btnPrev.disabled) { prevPage(); } }
  if (e.altKey && e.key === 'ArrowRight') { if (!els.btnNext.disabled) { nextPage(); } }
});

// ── Helpers ───────────────────────────────────────────────────────────────────

function esc(s) {
  if (s == null) { return ''; }
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

function escHighlight(s, start, end) {
  return esc(s.slice(0, start)) +
    '<span class="match">' + esc(s.slice(start, end)) + '</span>' +
    esc(s.slice(end));
}

/// Highlight match text in displayed content.
/// Strategy: try stored match text first, fallback to current search query.
/// Always does case-insensitive search in the ACTUAL displayed content —
/// never trusts stored byte/char offsets blindly.
function highlightInContent(displayContent, fm) {
  // Try to extract the match text from stored search result
  let matchText = '';
  try {
    if (fm && fm.content && fm.match_start < fm.match_end) {
      matchText = fm.content.slice(fm.match_start, fm.match_end);
    }
  } catch (_) {}

  // Fallback: use current search query directly
  if (!matchText) {
    matchText = els.search.value.trim();
  }

  if (!matchText) { return esc(displayContent); }

  // Case-insensitive search in displayed content — handles pretty vs raw differences
  const lowerDisplay = displayContent.toLowerCase();
  const lowerMatch   = matchText.toLowerCase();

  // Use proportional position if we have reference coords, else find first occurrence
  let pos = -1;
  if (fm && fm.content && fm.content.length > 0) {
    pos = findMatchInPretty(displayContent, lowerMatch, fm.match_start, fm.content.length);
  }
  if (pos < 0) {
    pos = lowerDisplay.indexOf(lowerMatch);
  }

  return pos >= 0
    ? escHighlight(displayContent, pos, pos + matchText.length)
    : esc(displayContent);
}

// Find the occurrence of searchText (lowercase) in pretty (lowercase) closest to targetProportion.
function findMatchInPretty(pretty, searchTextLower, rawStart, rawLen) {
  if (!searchTextLower || !pretty) { return -1; }
  const prettyLower = pretty.toLowerCase();
  const targetPos   = rawLen > 0 ? Math.floor((rawStart / rawLen) * pretty.length) : 0;
  let best = -1, bestDist = Infinity, from = 0;
  while (true) {
    const pos = prettyLower.indexOf(searchTextLower, from);
    if (pos === -1) { break; }
    const dist = Math.abs(pos - targetPos);
    if (dist < bestDist) { bestDist = dist; best = pos; }
    from = pos + 1;
  }
  return best;
}

// ── Start ─────────────────────────────────────────────────────────────────────

loadPage(state.offset);
