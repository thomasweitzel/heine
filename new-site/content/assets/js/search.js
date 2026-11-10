/*
 * Canonical source: `starter-site/kits/starter/assets/js/search.js` in Heine.
 *
 * This file is intentionally copied into independently deployable sites. Edit
 * the canonical source first, then replace each maintained copy as a whole.
 * Do not add site-specific behavior to a copied version: templates provide
 * localized labels and CSS owns the visual design.
 *
 * The script consumes Heine search-index format 3. It requires `.search-form`,
 * `input[name=q]`, `.search-status`, `.search-results`, and
 * `.search-pagination`. The form supplies `data-search-url`, status labels,
 * plural result-count labels, `data-no-results`, and `data-failure`.
 * `data-results-per-page`, `data-context-before`, `data-context-after`,
 * their `-max` counterparts, `data-draft-marker`, and `.search-clear` are
 * optional. Results use
 * `search-result`, `search-result-heading`,
 * `search-result-title`, and `search-result-excerpt`. Its pagination
 * container must also carry
 * `.pagination`: links, the current-page span, and gap spans use the same
 * semantic HTML as every other pagination control. Consuming sites style that
 * shared pagination structure with their own CSS.
 *
 * Authored search data always becomes DOM text nodes, never HTML strings.
 */
(() => {
  const form = document.querySelector(".search-form");
  if (!form) return;

  const input = form.querySelector("input[name=q]");
  const clearButton = form.querySelector(".search-clear");
  const status = document.querySelector(".search-status");
  const results = document.querySelector(".search-results");
  const navigation = document.querySelector(".search-pagination");
  const labels = form.dataset;
  const integerSetting = (name, fallback, minimum) => {
    const value = Number(form.dataset[name] || fallback);
    if (Number.isSafeInteger(value) && value >= minimum) return value;
    console.error(`search ${name} must be an integer no less than ${minimum}`);
    return fallback;
  };
  const pageSize = integerSetting("resultsPerPage", 10, 1);
  const contextBefore = integerSetting("contextBefore", 80, 0);
  const contextAfter = integerSetting("contextAfter", 160, 0);
  const contextMaximum = (name, preferred, fallback) => {
    const value = form.dataset[name];
    if (value === undefined || value === "") return Math.max(preferred, fallback);
    const parsed = Number(value);
    if (Number.isSafeInteger(parsed) && parsed >= preferred) return parsed;
    console.error(`search ${name} must be an integer no less than its preferred context`);
    return Math.max(preferred, fallback);
  };
  const contextBeforeMaximum = contextMaximum("contextBeforeMax", contextBefore, 100);
  const contextAfterMaximum = contextMaximum("contextAfterMax", contextAfter, 200);
  const windowSize = 1;
  const locale = document.documentElement.lang;
  let documentsPromise;
  let documentsLoading = false;
  let debounce;
  let renderGeneration = 0;

  const unspacedCharacter = /^[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\p{Script=Thai}\p{Script=Lao}\p{Script=Khmer}\p{Script=Myanmar}\u3000-\u303f\uff00-\uffef]$/u;
  const wordCharacter = /[\p{L}\p{N}\p{M}]/u;
  const joinUnspacedLineBreaks = (value) => [...value].map((character, index, characters) => (
    character === "\n"
      && unspacedCharacter.test(characters[index - 1] || "")
      && unspacedCharacter.test(characters[index + 1] || "")
      ? ""
      : character
  )).join("");
  const normalize = (value) => joinUnspacedLineBreaks(value.normalize("NFC")
      .toLocaleLowerCase(locale)
  ).replace(/\s+/gu, " ").trim();
  const pluralRules = new Intl.PluralRules(locale);
  const numberFormatter = new Intl.NumberFormat(locale);
  const queryState = () => new URLSearchParams(window.location.search);
  const pageNumber = (value, total) => Math.min(Math.max(Number.parseInt(value, 10) || 1, 1), total || 1);

  function syncClearButton() {
    if (clearButton) clearButton.hidden = !input.value;
  }

  function documents() {
    if (!documentsPromise) {
      documentsLoading = true;
      documentsPromise = fetch(form.dataset.searchUrl).then((response) => {
        if (!response.ok) throw new Error("search index request failed");
        return response.json();
      }).then((index) => {
        if (index.format !== 3 || !Array.isArray(index.documents)) {
          throw new Error("unsupported search index format");
        }
        return index.documents.map((doc, searchOrder) => ({
          ...doc,
          // Heine writes this index in page-ID order. Keep that order for ties.
          searchOrder,
          titleSearchText: normalize(doc.title || ""),
          searchSections: doc.sections.map((section) => ({
            ...section,
            displayText: joinUnspacedLineBreaks(section.text.normalize("NFC"))
              .replace(/\s+/gu, " ")
              .trim(),
            searchText: normalize(section.text),
          })),
        }));
      }).catch((error) => {
        documentsPromise = undefined;
        throw error;
      }).finally(() => {
        documentsLoading = false;
      });
    }
    return documentsPromise;
  }

  function characterBefore(value, index) {
    const preceding = value.charCodeAt(index - 1);
    if (preceding >= 0xdc00 && preceding <= 0xdfff && index > 1) {
      const leading = value.charCodeAt(index - 2);
      if (leading >= 0xd800 && leading <= 0xdbff) return value.slice(index - 2, index);
    }
    return value[index - 1] || "";
  }

  function characterAt(value, index) {
    const codePoint = value.codePointAt(index);
    return codePoint === undefined ? "" : String.fromCodePoint(codePoint);
  }

  function startsWord(previous, current) {
    // Unspaced scripts have no word separators, so each character may begin a match.
    return !wordCharacter.test(previous) || unspacedCharacter.test(current);
  }

  function matchPositions(text, fragment) {
    const positions = [];
    let position = text.indexOf(fragment);
    while (position >= 0) {
      if (startsWord(characterBefore(text, position), characterAt(text, position))) {
        positions.push(position);
      }
      position = text.indexOf(fragment, position + fragment.length);
    }
    return positions;
  }

  function characterMatchPositions(characters, fragment) {
    const pattern = Array.from(fragment);
    const positions = [];
    for (let start = 0; start <= characters.length - pattern.length; start += 1) {
      if (startsWord(characters[start - 1] || "", characters[start])
        && pattern.every((value, offset) => characters[start + offset] === value)) {
        positions.push(start);
      }
    }
    return positions;
  }

  function occurrences(text, fragment) {
    return matchPositions(text, fragment).length;
  }

  function matchingSections(sections, fragments) {
    return sections.map((section, order) => {
      const counts = fragments.map((fragment) => occurrences(section.searchText, fragment));
      return {
        section,
        order,
        counts,
        coverage: counts.filter(Boolean).length,
        occurrences: counts.reduce((total, count) => total + count, 0),
      };
    }).filter((match) => match.coverage).sort((left, right) => (
      right.coverage - left.coverage
      || right.occurrences - left.occurrences
      || left.order - right.order
    ));
  }

  function scoreDocument(doc, fragments) {
    const sections = matchingSections(doc.searchSections, fragments);
    let score = 0;
    for (const [index, fragment] of fragments.entries()) {
      if (occurrences(doc.titleSearchText, fragment)) score += 3;
      else if (sections.some((candidate) => candidate.counts[index])) score += 1;
      else return null;
    }
    return {
      score: score + Math.min(sections[0]?.occurrences || 0, 3),
      section: sections[0]?.section,
    };
  }

  function navigationPages(current, total) {
    const pages = new Set([1, total]);
    for (let page = Math.max(1, current - windowSize); page <= Math.min(total, current + windowSize); page += 1) pages.add(page);
    return [...pages].sort((left, right) => left - right);
  }

  function searchUrl(query, page) {
    const parameters = queryState();
    if (query) parameters.set("q", query); else parameters.delete("q");
    if (page > 1) parameters.set("page", page); else parameters.delete("page");
    return `${window.location.pathname}${parameters.size ? `?${parameters}` : ""}`;
  }

  function setUrl(query, page, replace) {
    window.history[replace ? "replaceState" : "pushState"]({}, "", searchUrl(query, page));
  }

  function resultCount(count) {
    const category = pluralRules.select(count);
    const name = `results${category[0].toUpperCase()}${category.slice(1)}`;
    const template = labels[name] || labels.resultsOther;
    return template.replace("__count__", numberFormatter.format(count));
  }

  function passageCandidates(section, fragments) {
    const characters = Array.from(section.displayText);
    const normalizedCharacters = Array.from(section.searchText);
    // Some case mappings expand one character. The full section still matches,
    // but a code-point offset can no longer name the displayed text reliably.
    // Omit this excerpt rather than publishing an unbounded or misleading one.
    if (characters.length !== normalizedCharacters.length) {
      return [];
    }
    const matches = fragments.flatMap((fragment) => {
      const length = Array.from(fragment).length;
      return characterMatchPositions(normalizedCharacters, fragment)
        .map((start) => ({ start, end: start + length }));
    }).sort((left, right) => left.start - right.start || right.end - left.end);
    const positions = [];
    for (const match of matches) {
      const previous = positions.at(-1);
      if (previous?.start === match.start) {
        previous.end = Math.max(previous.end, match.end);
      } else {
        positions.push(match);
      }
    }
    if (!positions.length) return [];

    const bestMatch = () => {
      let first = 0;
      let after = 0;
      let best;
      for (const match of positions) {
        while (first < positions.length && positions[first].start < match.start - contextBefore) first += 1;
        while (after < positions.length && positions[after].start <= match.end + contextAfter) after += 1;
        if (!best || after - first > best.nearby) {
          best = { match, nearby: after - first };
        }
      }
      return best;
    };
    const { match } = bestMatch();
    let start = Math.max(0, match.start - contextBefore);
    let end = Math.min(characters.length, match.end + contextAfter);
    const earliest = Math.max(0, match.start - contextBeforeMaximum);
    const latest = Math.min(characters.length, match.end + contextAfterMaximum);
    while (start > earliest && wordCharacter.test(characters[start - 1])
      && !unspacedCharacter.test(characters[start - 1]) && !unspacedCharacter.test(characters[start])) {
      start -= 1;
    }
    while (end < latest && wordCharacter.test(characters[end])
      && !unspacedCharacter.test(characters[end - 1]) && !unspacedCharacter.test(characters[end])) {
      end += 1;
    }
    return [{
      section,
      text: `${start ? "…" : ""}${characters.slice(start, end).join("")}${end < characters.length ? "…" : ""}`,
    }];
  }

  function bestPassage(section, fragments) {
    if (!section) return undefined;
    return passageCandidates(section, fragments)[0];
  }

  function resultTarget(doc, section) {
    const target = new URL(doc.url, window.location.origin);
    if (section?.fragment) target.hash = section.fragment;
    return `${target.pathname}${target.search}${target.hash}`;
  }

  function resultLink(result, fragments) {
    const { doc, section } = result;
    const passage = bestPassage(section, fragments);
    const item = document.createElement("li");
    const anchor = document.createElement("a");
    anchor.className = "search-result";
    anchor.href = resultTarget(doc, passage?.section);
    const heading = document.createElement("h2");
    heading.className = "search-result-heading";
    if (doc.draft && labels.draftMarker) {
      const marker = document.createElement("span");
      marker.className = "draft-marker";
      marker.textContent = labels.draftMarker;
      heading.append(marker);
    }
    const title = document.createElement("span");
    title.className = "search-result-title";
    title.textContent = doc.title || doc.url;
    heading.append(title);
    anchor.append(heading);
    const excerptText = passage?.text || doc.description;
    if (excerptText) {
      const excerpt = document.createElement("p");
      excerpt.className = "search-result-excerpt";
      excerpt.textContent = excerptText;
      anchor.append(excerpt);
    }
    item.append(anchor);
    return item;
  }

  function navigationLink(query, page) {
    const node = document.createElement("a");
    node.href = searchUrl(query, page);
    node.textContent = page;
    node.addEventListener("click", (event) => {
      event.preventDefault();
      setUrl(query, page, false);
      render(query, page);
    });
    return node;
  }

  function renderNavigation(query, current, total) {
    navigation.replaceChildren();
    navigation.hidden = total <= 1;
    let previous = 0;
    for (const page of navigationPages(current, total)) {
      if (page > previous + 1) {
        const gap = document.createElement("span");
        gap.textContent = "…";
        gap.setAttribute("aria-hidden", "true");
        navigation.append(gap);
      }
      const node = page === current ? document.createElement("span") : navigationLink(query, page);
      if (page === current) {
        node.textContent = page;
        node.setAttribute("aria-current", "page");
      }
      navigation.append(node);
      previous = page;
    }
  }

  async function render(query, requestedPage) {
    const generation = ++renderGeneration;
    const normalized = normalize(query);
    if (!normalized) {
      results.replaceChildren();
      navigation.replaceChildren();
      navigation.hidden = true;
      status.textContent = "";
      return;
    }
    try {
      const fragments = normalized.split(" ").filter(Boolean);
      const source = documents();
      if (documentsLoading) status.textContent = labels.loading;
      const documentsForSearch = await source;
      if (generation !== renderGeneration) return;
      const matching = documentsForSearch.map((doc) => {
        const match = scoreDocument(doc, fragments);
        return match && { doc, ...match };
      }).filter(Boolean)
        .sort((left, right) => right.score - left.score || left.doc.searchOrder - right.doc.searchOrder);
      if (generation !== renderGeneration) return;
      const total = Math.max(1, Math.ceil(matching.length / pageSize));
      const page = pageNumber(requestedPage, total);
      if (page !== requestedPage) setUrl(query, page, true);
      results.replaceChildren(...matching.slice((page - 1) * pageSize, page * pageSize)
        .map((result) => resultLink(result, fragments)));
      status.textContent = matching.length ? resultCount(matching.length) : labels.noResults;
      renderNavigation(query, page, total);
    } catch (_) {
      if (generation !== renderGeneration) return;
      results.replaceChildren();
      navigation.replaceChildren();
      navigation.hidden = true;
      status.textContent = labels.failure;
    }
  }

  function loadFromUrl() {
    const parameters = queryState();
    const query = parameters.get("q") || "";
    input.value = query;
    syncClearButton();
    render(query, pageNumber(parameters.get("page"), Number.MAX_SAFE_INTEGER));
  }

  input.addEventListener("focus", () => { documents().catch(() => {}); }, { once: true });
  input.addEventListener("input", () => {
    syncClearButton();
    window.clearTimeout(debounce);
    debounce = window.setTimeout(() => {
      setUrl(input.value, 1, true);
      render(input.value, 1);
    }, 150);
  });
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    setUrl(input.value, 1, false);
    render(input.value, 1);
  });
  clearButton?.addEventListener("click", () => {
    window.clearTimeout(debounce);
    input.value = "";
    syncClearButton();
    setUrl("", 1, true);
    render("", 1);
    input.focus();
  });
  window.addEventListener("popstate", loadFromUrl);
  loadFromUrl();
})();
