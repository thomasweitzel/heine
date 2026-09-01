(() => {
  const form = document.querySelector(".search-form");
  if (!form) return;

  const input = form.querySelector("input[name=q]");
  const status = document.querySelector(".search-status");
  const results = document.querySelector(".search-results");
  const navigation = document.querySelector(".search-pagination");
  const labels = form.dataset;
  const pageSize = 10;
  const windowSize = 1;
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
      .toLocaleLowerCase(document.documentElement.lang)
  ).replace(/\s+/gu, " ").trim();
  const pluralRules = new Intl.PluralRules(document.documentElement.lang);
  const queryState = () => new URLSearchParams(window.location.search);
  const pageNumber = (value, total) => Math.min(Math.max(Number.parseInt(value, 10) || 1, 1), total || 1);

  function documents() {
    if (!documentsPromise) {
      documentsLoading = true;
      documentsPromise = fetch(form.dataset.searchUrl).then((response) => {
        if (!response.ok) throw new Error("search index request failed");
        return response.json();
      }).then((index) => index.documents.map((doc) => ({
        ...doc,
        searchFields: [doc.title, doc.summary ?? doc.description, doc.text]
          .map((value) => normalize(value || "")),
      }))).catch((error) => {
        documentsPromise = undefined;
        throw error;
      }).finally(() => {
        documentsLoading = false;
      });
    }
    return documentsPromise;
  }

  function scoreDocument(doc, fragments) {
    let score = 0;
    for (const fragment of fragments) {
      const weight = doc.searchFields.findIndex((field) => field.includes(fragment));
      if (weight < 0) return null;
      score += [3, 2, 1][weight];
    }
    return score;
  }

  function navigationPages(current, total) {
    const pages = new Set();
    for (let page = 1; page <= Math.min(total, windowSize + 1); page += 1) pages.add(page);
    for (let page = Math.max(1, current - windowSize); page <= Math.min(total, current + windowSize); page += 1) pages.add(page);
    for (let page = Math.max(1, total - windowSize); page <= total; page += 1) pages.add(page);
    return [...pages].sort((left, right) => left - right);
  }

  function setUrl(query, page, replace) {
    const parameters = queryState();
    if (query) parameters.set("q", query); else parameters.delete("q");
    if (page > 1) parameters.set("page", page); else parameters.delete("page");
    const url = `${window.location.pathname}${parameters.size ? `?${parameters}` : ""}`;
    window.history[replace ? "replaceState" : "pushState"]({}, "", url);
  }

  function resultCount(count) {
    const category = pluralRules.select(count);
    const name = `results${category[0].toUpperCase()}${category.slice(1)}`;
    const template = labels[name] || labels.resultsOther;
    return template.replace("__count__", String(count));
  }

  function link(result) {
    const item = document.createElement("li");
    const anchor = document.createElement("a");
    anchor.href = result.url;
    const title = document.createElement("span");
    title.className = "page-list-title";
    title.textContent = result.title || result.url;
    anchor.append(title);
    const summary = result.summary ?? result.description;
    if (summary) {
      const copy = document.createElement("span");
      copy.className = "page-list-summary";
      copy.textContent = summary;
      anchor.append(copy);
    }
    item.append(anchor);
    return item;
  }

  function navigationLink(query, page) {
    const node = document.createElement("a");
    node.href = "#";
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
      const matching = (await source).map((doc) => ({ doc, score: scoreDocument(doc, fragments) }))
        .filter((result) => result.score !== null)
        .sort((left, right) => right.score - left.score);
      const total = Math.max(1, Math.ceil(matching.length / pageSize));
      const page = pageNumber(requestedPage, total);
      if (page !== requestedPage) setUrl(query, page, true);
      results.replaceChildren(...matching.slice((page - 1) * pageSize, page * pageSize).map((result) => link(result.doc)));
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
    render(query, pageNumber(parameters.get("page"), Number.MAX_SAFE_INTEGER));
  }

  input.addEventListener("focus", () => { documents().catch(() => {}); }, { once: true });
  input.addEventListener("input", () => {
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
  window.addEventListener("popstate", loadFromUrl);
  loadFromUrl();
})();
