/* global acquireVsCodeApi, GLANCE_CONFIG */

const vscode = acquireVsCodeApi();
const { totalLines, fileSizeMb, format, isJsonl, isCsv } = window.GLANCE_CONFIG;

const PAGE_RAW = 200;
const PAGE_PRETTY = 10;

let offset = 0;
let isSearching = false;
let useRegex = false;
let usePretty = false;
let searchTimer = null;

// Store current lines in memory so click handlers can reference content by index
// instead of embedding content strings in onclick attributes (which CSP blocks).
let currentLines = [];

// Search match navigation state
let allMatches = [];      // all search results for prev/next navigation
let matchIdx = -1;        // -1 = showing results list, >=0 = in match context mode
let focusedLine = -1;     // line number to highlight in file context
let savedTotalFound = 0;  // original total found (may be > allMatches.length if truncated)
let savedTruncated = false;
let searchGeneration = 0; // increments each search — prevents stale count from old search

// ── Init ────────────────────────────────────────────────────────────────

document.getElementById('info').textContent =
  totalLines.toLocaleString() + ' lines • ' + fileSizeMb + ' MB • ' + format.toUpperCase();

if (isJsonl) {
  document.getElementById('btn-pretty').classList.add('visible');
}

document.getElementById('btn-regex').addEventListener('click', toggleRegex);
document.getElementById('btn-pretty').addEventListener('click', togglePretty);
document.getElementById('btn-prev').addEventListener('click', prevPage);
document.getElementById('btn-next').addEventListener('click', nextPage);
document.getElementById('btn-match-prev').addEventListener('click', prevMatch);
document.getElementById('btn-match-next').addEventListener('click', nextMatch);

// ── Event delegation for #lines (no inline onclick needed) ──────────────

document.getElementById('lines').addEventListener('click', function(e) {
  const target = e.target;

  // Line number: in search results list → jump to match; otherwise → copy
  const lineNum = target.closest('.line-num, .row-num');
  if (lineNum) {
    const idx = parseInt(lineNum.dataset.idx, 10);
    if (!isNaN(idx) && currentLines[idx]) {
      if (matchIdx === -1 && allMatches.length > 0 && isSearching) {
        jumpToMatch(idx);
      } else {
        copyText(currentLines[idx].content);
      }
    }
    return;
  }

  // JSON card header
  const cardHeader = target.closest('.json-card-header');
  if (cardHeader) {
    if (cardHeader.classList.contains('search-nav')) {
      // Search result card → navigate to match context
      const idx = parseInt(cardHeader.dataset.idx, 10);
      if (!isNaN(idx)) { jumpToMatch(idx); }
    } else if (cardHeader.dataset.cardId) {
      // Raw card → toggle collapse
      toggleRaw(cardHeader.dataset.cardId);
    } else {
      // Pretty card → copy content
      const idx = parseInt(cardHeader.dataset.idx, 10);
      if (!isNaN(idx) && currentLines[idx]) { copyText(currentLines[idx].content); }
    }
    return;
  }

  // Raw copy button
  const copyBtn = target.closest('.raw-copy-btn');
  if (copyBtn) {
    const idx = parseInt(copyBtn.dataset.idx, 10);
    if (!isNaN(idx) && currentLines[idx]) {
      copyText(currentLines[idx].content);
    }
  }
});

// ── Toggles ────────────────────────────────────────────────────────────

function toggleRegex() {
  useRegex = !useRegex;
  document.getElementById('btn-regex').classList.toggle('active', useRegex);
  const input = document.getElementById('search');
  input.classList.toggle('regex-active', useRegex);
  input.placeholder = useRegex ? 'Regex pattern...' : 'Search in file...';
  const q = input.value.trim();
  if (q) { triggerSearch(q); }
}

function togglePretty() {
  usePretty = !usePretty;
  document.getElementById('btn-pretty').classList.toggle('active', usePretty);
  if (allMatches.length > 0 && matchIdx === -1) {
    // Re-render search results list with new pretty setting
    renderSearch(allMatches, savedTotalFound, savedTruncated);
  } else {
    // Reload current page (works for both normal mode and match context)
    const ps = pageSize();
    const pageOffset = focusedLine >= 0
      ? Math.max(0, focusedLine - Math.floor(ps / 2))
      : offset;
    vscode.postMessage({ type: 'read', offset: pageOffset, limit: ps, pretty: usePretty });
  }
}

function toggleRaw(cardId) {
  const preview = document.getElementById(cardId + '-preview');
  const full    = document.getElementById(cardId + '-full');
  const arrow   = document.getElementById(cardId + '-arrow');
  if (!preview || !full || !arrow) { return; }
  const isOpen = full.style.display !== 'none';
  preview.style.display = isOpen ? '' : 'none';
  full.style.display    = isOpen ? 'none' : '';
  arrow.textContent     = isOpen ? '▶ Show' : '▼ Hide';
}

// ── Navigation ──────────────────────────────────────────────────────────

function pageSize() { return usePretty ? PAGE_PRETTY : PAGE_RAW; }

function loadPage(newOffset) {
  const ps = pageSize();
  offset = Math.max(0, Math.min(newOffset, Math.max(0, totalLines - ps)));
  isSearching = false;
  matchIdx = -1;
  focusedLine = -1;
  allMatches = [];
  savedTotalFound = 0;
  savedTruncated = false;
  document.getElementById('search').value = '';
  document.getElementById('search').classList.remove('regex-active');
  document.getElementById('match-count').textContent = '';
  document.getElementById('match-nav-info').textContent = '';
  document.getElementById('btn-match-prev').disabled = true;
  document.getElementById('btn-match-next').disabled = true;
  vscode.postMessage({ type: 'read', offset: offset, limit: ps, pretty: usePretty });
  updateButtons();
}

// ── Page navigation (always navigates pages, independent of search) ──────

function prevPage() { loadPage(offset - pageSize()); }
function nextPage() { loadPage(offset + pageSize()); }

function updateButtons() {
  const ps = pageSize();
  document.getElementById('btn-prev').disabled = offset <= 0;
  document.getElementById('btn-next').disabled = offset + ps >= totalLines;
  document.getElementById('btn-prev').textContent = '← Prev';
  document.getElementById('btn-next').textContent = 'Next →';
}

// ── Match navigation (separate, only active when search has results) ──────

function prevMatch() {
  if (matchIdx > 0) {
    jumpToMatch(matchIdx - 1);
  } else if (matchIdx === 0) {
    // Back to results list
    matchIdx = -1;
    focusedLine = -1;
    renderSearch(allMatches, savedTotalFound, savedTruncated);
  }
}

function nextMatch() {
  if (matchIdx === -1 && allMatches.length > 0) {
    jumpToMatch(0);
  } else if (matchIdx >= 0 && matchIdx < allMatches.length - 1) {
    jumpToMatch(matchIdx + 1);
  }
}

function updateMatchNav() {
  const hasPrev = matchIdx >= 0;
  const hasNext = matchIdx === -1 ? allMatches.length > 0 : matchIdx < allMatches.length - 1;
  document.getElementById('btn-match-prev').disabled = !hasPrev;
  document.getElementById('btn-match-next').disabled = !hasNext;
  const info = document.getElementById('match-nav-info');
  if (allMatches.length > 0) {
    info.textContent = matchIdx >= 0
      ? (matchIdx + 1) + ' / ' + allMatches.length
      : allMatches.length + ' matches';
  } else {
    info.textContent = '';
  }
}

function jumpToMatch(idx) {
  if (idx < 0 || idx >= allMatches.length) { return; }
  matchIdx = idx;
  focusedLine = allMatches[idx].line_number;
  const ps = pageSize();
  const pageOffset = Math.max(0, focusedLine - Math.floor(ps / 2));
  vscode.postMessage({ type: 'read', offset: pageOffset, limit: ps, pretty: usePretty });
  updateMatchNav();
}

// ── Go-to-line ──────────────────────────────────────────────────────────

document.getElementById('goto-input').addEventListener('keydown', function(e) {
  if (e.key !== 'Enter') { return; }
  const n = parseInt(this.value, 10);
  if (isNaN(n) || n < 1 || n > totalLines) {
    this.classList.add('goto-error');
    setTimeout(function() { document.getElementById('goto-input').classList.remove('goto-error'); }, 800);
    return;
  }
  this.classList.remove('goto-error');
  this.value = '';
  // Use loadPage so offset is synced and pretty mode is respected
  loadPage(n - 1);
});

// ── Search ───────────────────────────────────────────────────────────────

function triggerSearch(q) {
  isSearching = true;
  matchIdx = -1;
  focusedLine = -1;
  allMatches = [];
  savedTotalFound = 0;
  savedTruncated = false;
  searchGeneration++;
  const gen = searchGeneration;
  document.getElementById('match-count').textContent = 'searching…';
  document.getElementById('match-nav-info').textContent = '';
  document.getElementById('btn-match-prev').disabled = true;
  document.getElementById('btn-match-next').disabled = true;
  updateButtons();
  vscode.postMessage({ type: 'search', query: q, useRegex: useRegex });
  vscode.postMessage({ type: 'count', query: q, useRegex: useRegex, gen: gen });
}

document.getElementById('search').addEventListener('input', function() {
  clearTimeout(searchTimer);
  const q = this.value.trim();
  if (!q) { loadPage(offset); return; }
  searchTimer = setTimeout(function() { triggerSearch(document.getElementById('search').value.trim()); }, 300);
});

// ── Clipboard ────────────────────────────────────────────────────────────

function copyText(content) {
  navigator.clipboard.writeText(content).then(function() { showToast('Copied!'); });
}

function showToast(msg) {
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.classList.add('show');
  setTimeout(function() { t.classList.remove('show'); }, 1500);
}

// ── Message handler ──────────────────────────────────────────────────────

window.addEventListener('message', function(e) {
  const msg = e.data;
  if (msg.type === 'lines')          { renderLines(msg.lines, msg.offset, msg.total_lines); }
  if (msg.type === 'search_results') { renderSearch(msg.results, msg.total_found, msg.truncated); }
  if (msg.type === 'count_result') {
    // Only apply if generation matches — prevents stale count from old search
    if (msg.gen === undefined || msg.gen === searchGeneration) {
      document.getElementById('match-count').textContent = msg.count.toLocaleString() + ' total';
    }
  }
  if (msg.type === 'error')          { showError(msg.message); }
});

// ── Renderers ────────────────────────────────────────────────────────────

function renderLines(lines, off, total) {
  currentLines = lines;
  offset = off; // sync offset from server so Prev/Next work correctly after goto
  const el = document.getElementById('lines');
  if (!lines.length) {
    el.innerHTML = '<div id="status">No lines found.</div>';
    updateButtons();
    return;
  }

  if (isCsv && lines[0].fields) {
    el.innerHTML = renderCsvTable(lines);
  } else if (usePretty && isJsonl) {
    el.innerHTML = lines.map(function(l, idx) {
      const isFocused = focusedLine >= 0 && l.number === focusedLine;
      const focusedMatch = isFocused && matchIdx >= 0 ? allMatches[matchIdx] : null;
      return renderJsonCard(l, idx, focusedMatch);
    }).join('');
  } else {
    el.innerHTML = lines.map(function(l, idx) {
      const isFocused = focusedLine >= 0 && l.number === focusedLine;
      const match = isFocused && matchIdx >= 0 ? allMatches[matchIdx] : null;
      const content = match
        ? escHighlight(l.content, match.match_start, match.match_end)
        : esc(l.content);
      return '<div class="line">' +
        '<span class="line-num" data-idx="' + idx + '">' + (l.number + 1) + '</span>' +
        '<span class="line-content">' + content + '</span>' +
        '</div>';
    }).join('');
  }

  el.scrollTop = 0; // reset after innerHTML so scroll is correct

  // Highlight and scroll to focused line (after match navigation)
  if (focusedLine >= 0) {
    let found = false;
    // Raw/normal mode: look for .line with matching .line-num text
    const allLineEls = el.querySelectorAll('.line');
    for (let i = 0; i < allLineEls.length; i++) {
      const numEl = allLineEls[i].querySelector('.line-num');
      if (numEl && parseInt(numEl.textContent, 10) === focusedLine + 1) {
        allLineEls[i].classList.add('line-focused');
        allLineEls[i].scrollIntoView({ block: 'center' });
        found = true;
        break;
      }
    }
    // Pretty mode: look for .json-card with data-line-num attribute
    if (!found) {
      const allCards = el.querySelectorAll('.json-card[data-line-num]');
      for (let i = 0; i < allCards.length; i++) {
        if (parseInt(allCards[i].dataset.lineNum, 10) === focusedLine) {
          allCards[i].classList.add('line-focused');
          allCards[i].scrollIntoView({ block: 'center' });
          break;
        }
      }
    }
  }

  const end = Math.min(off + lines.length, total);
  document.getElementById('page-info').textContent =
    'Lines ' + (off + 1).toLocaleString() + '–' + end.toLocaleString() + ' of ' + total.toLocaleString();
  updateButtons();
}

function renderJsonCard(l, idx, focusedMatch) {
  const isPretty = l.content.includes('\n');

  if (isPretty) {
    let body;
    if (focusedMatch) {
      // match_start/end refer to raw content positions; find in pretty using proportion
      const rawMatchText = focusedMatch.content.slice(focusedMatch.match_start, focusedMatch.match_end);
      const pos = findMatchInPretty(l.content, rawMatchText, focusedMatch.match_start, focusedMatch.content.length);
      body = pos >= 0
        ? escHighlight(l.content, pos, pos + rawMatchText.length)
        : esc(l.content);
    } else {
      body = esc(l.content);
    }
    return '<div class="json-card" data-line-num="' + l.number + '">' +
      '<div class="json-card-header" data-idx="' + idx + '" title="Click to copy">' +
      '<span>Line ' + (l.number + 1) + '</span><span>⎘</span>' +
      '</div>' +
      '<div class="json-card-body">' + body + '</div>' +
      '</div>';
  }

  const PREVIEW_LEN = 150;
  const cardId = 'raw-' + l.number;
  const preview = l.content.length > PREVIEW_LEN
    ? esc(l.content.slice(0, PREVIEW_LEN)) + '<span class="raw-ellipsis">… (' + l.content.length.toLocaleString() + ' chars)</span>'
    : esc(l.content);

  return '<div class="json-card raw" data-line-num="' + l.number + '">' +
    '<div class="json-card-header" data-card-id="' + cardId + '">' +
    '<span>⚠ Line ' + (l.number + 1) + ' — not valid JSON (possibly multi-line record)</span>' +
    '<span id="' + cardId + '-arrow">▶ Show</span>' +
    '</div>' +
    '<div class="json-card-body raw-preview" id="' + cardId + '-preview">' + preview + '</div>' +
    '<div class="json-card-body raw-full" id="' + cardId + '-full" style="display:none">' +
    esc(l.content) +
    '<br><button class="raw-copy-btn" data-idx="' + idx + '">Copy raw content</button>' +
    '</div>' +
    '</div>';
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

function renderSearch(results, totalFound, truncated) {
  allMatches = results;
  savedTotalFound = totalFound;
  savedTruncated = truncated;
  matchIdx = -1;
  focusedLine = -1;
  currentLines = results.map(function(r) { return { number: r.line_number, content: r.content }; });
  const el = document.getElementById('lines');
  if (!results.length) {
    el.innerHTML = '<div id="status">No matches found.</div>';
    document.getElementById('page-info').textContent = '0 matches';
    updateMatchNav();
    return;
  }
  el.scrollTop = 0;

  if (usePretty && isJsonl) {
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
  document.getElementById('page-info').textContent =
    totalFound.toLocaleString() + ' matches' + suffix;
  updateMatchNav();
  updateButtons();
}

function renderSearchCard(r, idx) {
  let displayContent = r.content;
  let hlStart = r.match_start;
  let hlEnd = r.match_end;

  if (r.content.trim().startsWith('{')) {
    try {
      const parsed = JSON.parse(r.content);
      const pretty = JSON.stringify(parsed, null, 2);
      const matchText = r.content.slice(r.match_start, r.match_end);
      const pos = findMatchInPretty(pretty, matchText, r.match_start, r.content.length);
      displayContent = pretty;
      hlStart = pos >= 0 ? pos : -1;
      hlEnd = pos >= 0 ? pos + matchText.length : -1;
    } catch (e) { /* keep raw if not valid JSON */ }
  }

  const body = (hlStart >= 0 && hlEnd > hlStart)
    ? escHighlight(displayContent, hlStart, hlEnd)
    : esc(displayContent);

  return '<div class="json-card">' +
    '<div class="json-card-header search-nav" data-idx="' + idx + '" title="Click to view in context">' +
    '<span>Line ' + (r.line_number + 1) + '</span><span>→ view</span>' +
    '</div>' +
    '<div class="json-card-body">' + body + '</div>' +
    '</div>';
}

function showError(msg) {
  document.getElementById('lines').innerHTML = '<div id="error-msg">Error: ' + esc(msg) + '</div>';
}

// ── Helpers ───────────────────────────────────────────────────────────────

// Find the occurrence of matchText in pretty that is closest to the
// proportionally-scaled position of the match in the raw content.
// This handles cases where matchText (e.g. "id") appears multiple times.
function findMatchInPretty(pretty, matchText, rawMatchStart, rawLength) {
  if (!matchText || !pretty) { return -1; }
  const targetPos = rawLength > 0
    ? Math.floor((rawMatchStart / rawLength) * pretty.length)
    : 0;
  let bestPos = -1;
  let bestDist = Infinity;
  let from = 0;
  while (true) {
    const pos = pretty.indexOf(matchText, from);
    if (pos === -1) { break; }
    const dist = Math.abs(pos - targetPos);
    if (dist < bestDist) { bestDist = dist; bestPos = pos; }
    from = pos + 1;
  }
  return bestPos;
}

function esc(s) {
  if (s == null) { return ''; }
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function escHighlight(s, start, end) {
  return esc(s.slice(0, start)) +
    '<span class="match">' + esc(s.slice(start, end)) + '</span>' +
    esc(s.slice(end));
}

// ── Keyboard shortcuts ────────────────────────────────────────────────────

document.addEventListener('keydown', function(e) {
  if (e.ctrlKey && e.key === 'g') { e.preventDefault(); document.getElementById('goto-input').focus(); }
  if (e.ctrlKey && e.key === 'f') { e.preventDefault(); document.getElementById('search').focus(); }
  if (e.ctrlKey && e.key === 'r') { e.preventDefault(); toggleRegex(); }
  if (e.key === 'Escape') {
    const s = document.getElementById('search');
    if (s.value) {
      s.value = '';
      loadPage(offset);
    }
  }
  // Enter / Shift+Enter → next/prev match when search has results
  if (e.key === 'Enter' && document.activeElement === document.getElementById('search')) {
    e.preventDefault();
    if (e.shiftKey) { prevMatch(); } else { nextMatch(); }
  }
  if (e.altKey && e.key === 'ArrowLeft')  { if (!document.getElementById('btn-prev').disabled) { prevPage(); } }
  if (e.altKey && e.key === 'ArrowRight') { if (!document.getElementById('btn-next').disabled) { nextPage(); } }
});

// ── Start ─────────────────────────────────────────────────────────────────

loadPage(0);
