document.addEventListener("DOMContentLoaded", () => {
    const style = document.createElement("style");
    style.textContent = `
        @keyframes __sonionFadeIn {
            from { opacity: 0; transform: translateY(6px); }
            to { opacity: 1; transform: translateY(0); }
        }
        html { animation: __sonionFadeIn 0.22s cubic-bezier(0.22, 1, 0.36, 1); }
        ::highlight(sonion-find) { background-color: #f5d90a; color: #000; }
        ::highlight(sonion-find-cur) { background-color: #ff9632; color: #000; }
    `;
    (document.head || document.documentElement).appendChild(style);
});

window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key || "").toLowerCase() === "f") {
        e.preventDefault();
        e.stopImmediatePropagation();
        window.ipc?.postMessage(JSON.stringify({shortcut: "f"}));
    }
}, true);

window.__sonionFind = (() => {
    let query = "";
    let ranges = [];
    let index = -1;

    const collect = (q) => {
        const needle = q.toLowerCase();
        const found = [];
        if (!needle) return found;
        const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT, {
            acceptNode: (node) => {
                const parent = node.parentElement;
                if (!parent || /^(SCRIPT|STYLE|NOSCRIPT|TEXTAREA)$/.test(parent.tagName)) return NodeFilter.FILTER_REJECT;
                return parent.getClientRects().length ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
            }
        });
        for (let node = walker.nextNode(); node && found.length < 1000; node = walker.nextNode()) {
            const text = node.data.toLowerCase();
            for (let i = text.indexOf(needle); i !== -1 && found.length < 1000; i = text.indexOf(needle, i + needle.length)) {
                try {
                    const range = new Range();
                    range.setStart(node, i);
                    range.setEnd(node, i + needle.length);
                    found.push(range);
                } catch (_) {}
            }
        }
        return found;
    };

    const find = (q, dir) => {
        if (!window.CSS || !CSS.highlights) return [0, 0];
        if (dir === 0 || q !== query) {
            query = q;
            ranges = collect(q);
            index = ranges.length ? 0 : -1;
        } else if (ranges.length) {
            index = (index + dir + ranges.length) % ranges.length;
        }
        CSS.highlights.set("sonion-find", new Highlight(...ranges));
        if (index < 0) {
            CSS.highlights.delete("sonion-find-cur");
            return [0, 0];
        }
        const current = new Highlight(ranges[index]);
        current.priority = 1;
        CSS.highlights.set("sonion-find-cur", current);
        ranges[index].startContainer.parentElement?.scrollIntoView({block: "center", inline: "nearest"});
        return [index + 1, ranges.length];
    };

    const clear = () => {
        query = "";
        ranges = [];
        index = -1;
        if (window.CSS && CSS.highlights) {
            CSS.highlights.delete("sonion-find");
            CSS.highlights.delete("sonion-find-cur");
        }
    };

    return {find, clear};
})();
