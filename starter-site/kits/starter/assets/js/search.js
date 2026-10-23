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
 * `data-draft-marker`, and `.search-clear` are optional. Results use
 * `search-result`, `search-result-heading`,
 * `search-result-title`, `search-result-contexts`, `search-result-context`,
 * and `search-result-summary`; pagination uses `search-pagination-link`,
 * `search-pagination-current`, and `search-pagination-gap`. Consuming sites
 * style those semantic classes with their own CSS.
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
  const contextBefore = integerSetting("contextBefore", 90, 0);
  const contextAfter = integerSetting("contextAfter", 170, 0);
  const windowSize = 1;
  const locale = document.documentElement.lang;
  let documentsPromise;
  let documentsLoading = false;
  let debounce;

  const unspacedCharacter = /^[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\p{Script=Thai}\p{Script=Lao}\p{Script=Khmer}\p{Script=Myanmar}\u3000-\u303f\uff00-\uffef]$/u;
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
          searchFields: [doc.title, doc.summary ?? doc.description]
            .map((value) => normalize(value || "")),
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

  function occurrences(text, fragment) {
    let count = 0;
    let position = text.indexOf(fragment);
    while (position >= 0) {
      count += 1;
      position = text.indexOf(fragment, position + fragment.length);
    }
    return count;
  }

  function matchingSections(sections, fragments) {
    return sections.map((section, order) => {
      const counts = fragments.map((fragment) => occurrences(section.searchText, fragment));
      return {
        section,
        order,
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
    const titleMatches = fragments.map((fragment) => doc.searchFields[0].includes(fragment));
    let score = 0;
    for (const fragment of fragments) {
      const weight = doc.searchFields.findIndex((field) => field.includes(fragment));
      if (weight >= 0) score += [3, 2][weight];
      else if (sections.some((candidate) => candidate.section.searchText.includes(fragment))) score += 1;
      else return null;
    }
    return {
      score: score + Math.min(sections[0]?.occurrences || 0, 3),
      sections: sections.slice(0, 2),
      summaryAddsMatch: fragments.some((fragment, index) => (
        !titleMatches[index] && doc.searchFields[1].includes(fragment)
      )),
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
    if (characters.length !== normalizedCharacters.length) {
      return [{ section, text: section.displayText }];
    }
    const positions = fragments.flatMap((fragment) => {
      const pattern = Array.from(fragment);
      const matches = [];
      for (let start = 0; start <= normalizedCharacters.length - pattern.length; start += 1) {
        if (pattern.every((value, offset) => normalizedCharacters[start + offset] === value)) {
          matches.push(start);
        }
      }
      return matches;
    }).sort((left, right) => left - right)
      .filter((position, index, values) => !index || position !== values[index - 1]);
    if (!positions.length) return [{ section, text: section.displayText }];

    const bestPosition = (excluded) => {
      let first = 0;
      let after = 0;
      let best;
      for (const position of positions) {
        while (first < positions.length && positions[first] < position - contextBefore) first += 1;
        while (after < positions.length && positions[after] <= position + contextAfter) after += 1;
        const overlaps = excluded !== undefined
          && position - contextBefore <= excluded + contextAfter
          && position + contextAfter >= excluded - contextBefore;
        if (!overlaps && (!best || after - first > best.nearby)) {
          best = { position, nearby: after - first };
        }
      }
      return best;
    };
    const selected = [bestPosition()];
    const second = bestPosition(selected[0].position);
    if (second) selected.push(second);

    return selected.map(({ position }) => {
      const start = Math.max(0, position - contextBefore);
      const end = Math.min(characters.length, position + contextAfter);
      return {
        section,
        text: `${start ? "…" : ""}${characters.slice(start, end).join("")}${end < characters.length ? "…" : ""}`,
      };
    });
  }

  function matchingPassages(sections, fragments) {
    const candidates = sections.map((match) => passageCandidates(match.section, fragments));
    const first = candidates.flatMap((passages) => passages.slice(0, 1));
    const further = candidates.flatMap((passages) => passages.slice(1));
    return first.concat(further).slice(0, 2);
  }

  function resultTarget(doc, section) {
    const target = new URL(doc.url, window.location.origin);
    if (section?.fragment) target.hash = section.fragment;
    return `${target.pathname}${target.search}${target.hash}`;
  }

  function appendResultLink(item, className, doc, section, text) {
    const anchor = document.createElement("a");
    anchor.className = className;
    anchor.href = resultTarget(doc, section);
    anchor.textContent = text;
    item.append(anchor);
  }

  function resultLink(result, fragments) {
    const { doc, sections, summaryAddsMatch } = result;
    const passages = matchingPassages(sections, fragments);
    const item = document.createElement("li");
    item.className = "search-result";
    const heading = document.createElement("h2");
    heading.className = "search-result-heading";
    if (doc.draft && labels.draftMarker) {
      const marker = document.createElement("span");
      marker.className = "draft-marker";
      marker.textContent = labels.draftMarker;
      heading.append(marker);
    }
    const anchor = document.createElement("a");
    anchor.className = "search-result-title";
    anchor.href = resultTarget(doc, passages[0]?.section);
    const title = document.createElement("span");
    title.textContent = doc.title || doc.url;
    anchor.append(title);
    heading.append(anchor);
    item.append(heading);
    if (passages.length) {
      const contexts = document.createElement("div");
      contexts.className = "search-result-contexts";
      for (const passage of passages) {
        appendResultLink(contexts, "search-result-context", doc, passage.section, passage.text);
      }
      item.append(contexts);
    } else if (summaryAddsMatch) {
      const summary = doc.summary ?? doc.description;
      appendResultLink(item, "search-result-summary", doc, undefined, summary);
    }
    return item;
  }

  function navigationLink(query, page) {
    const node = document.createElement("a");
    node.className = "search-pagination-link";
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
        gap.className = "search-pagination-gap";
        gap.textContent = "…";
        gap.setAttribute("aria-hidden", "true");
        navigation.append(gap);
      }
      const node = page === current ? document.createElement("span") : navigationLink(query, page);
      if (page === current) {
        node.className = "search-pagination-current";
        node.textContent = page;
        node.setAttribute("aria-current", "page");
      }
      navigation.append(node);
      previous = page;
    }
  }

  async function render(query, requestedPage) {
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
      const matching = (await source).map((doc) => {
        const match = scoreDocument(doc, fragments);
        return match && { doc, ...match };
      }).filter(Boolean)
        .sort((left, right) => right.score - left.score || left.doc.searchOrder - right.doc.searchOrder);
      const total = Math.max(1, Math.ceil(matching.length / pageSize));
      const page = pageNumber(requestedPage, total);
      if (page !== requestedPage) setUrl(query, page, true);
      results.replaceChildren(...matching.slice((page - 1) * pageSize, page * pageSize)
        .map((result) => resultLink(result, fragments)));
      status.textContent = matching.length ? resultCount(matching.length) : labels.noResults;
      renderNavigation(query, page, total);
    } catch (_) {
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
