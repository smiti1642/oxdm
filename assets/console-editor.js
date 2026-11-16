const input = document.getElementById("oxdm-console-input");
const paint = document.getElementById("oxdm-console-paint");
if (!input || !paint) return;

const initial = await dioxus.recv();
if (!input.value) input.value = initial;
input.setSelectionRange(input.value.length, input.value.length);
const editor = input.closest(".console-editor");
let composing = false;
let last = null;
const send = (action) => {
    const caret = input.selectionEnd ?? input.value.length;
    const signature = JSON.stringify([input.value, caret]);
    if (action === "change" && signature === last) return;
    last = signature;
    dioxus.send({ action, text: input.value, caret });
};
const align = () => {
    paint.style.transform = "translateX(" + -input.scrollLeft + "px)";
};
const change = () => {
    align();
    if (!composing) send("change");
};

for (const event of ["input", "click", "select", "keyup", "scroll"]) {
    input.addEventListener(event, change);
}
input.addEventListener("focus", () => send("focus"));
input.addEventListener("blur", () => send("blur"));
input.addEventListener("console-run", () => {
    if (!composing) send("run");
});
input.addEventListener("console-replace", () => {
    align();
    send("replace");
});
input.addEventListener("compositionstart", () => {
    composing = true;
    input.classList.add("console-composing");
});
input.addEventListener("compositionend", () => {
    composing = false;
    input.classList.remove("console-composing");
    change();
});
input.addEventListener("paste", (event) => {
    if (/[\r\n]/.test(event.clipboardData?.getData("text/plain") ?? "")) {
        event.preventDefault();
        send("multiline");
    }
});
const accept = (item) => {
    if (!item || item.parentElement.dataset.source !== input.value ||
        Number(item.dataset.caret) !== input.selectionEnd) {
        send("change");
        return;
    }
    input.setRangeText(item.dataset.replacement, Number(item.dataset.start),
                       Number(item.dataset.end), "end");
    input.focus();
    input.setAttribute("aria-expanded", "false");
    align();
    send("accept");
};
editor.addEventListener("mousedown", (event) => {
    if (event.target.closest("[role=option]")) event.preventDefault();
});
editor.addEventListener("click", (event) => {
    const item = event.target.closest("[role=option]");
    if (item) accept(item);
});
input.addEventListener("keydown", (event) => {
    const inComposition = composing || event.isComposing || event.keyCode === 229;
    if (!inComposition && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") return;
    const open = input.getAttribute("aria-expanded") === "true";
    if (!inComposition && !open && event.key === "Escape") return;
    event.stopPropagation();
    if (inComposition) return;
    const active = document.getElementById(input.getAttribute("aria-activedescendant"));
    if (open && (event.key === "Enter" || (event.key === "Tab" && !event.shiftKey))) {
        event.preventDefault();
        accept(active);
        return;
    }
    if (open && active && ["ArrowUp", "ArrowDown"].includes(event.key)) {
        const next = (event.key === "ArrowUp" ? active.previousElementSibling : active.nextElementSibling) || active;
        input.setAttribute("aria-activedescendant", next.id);
    }
    if (["ArrowUp", "ArrowDown", "Enter", "Escape", "F5"].includes(event.key)) {
        event.preventDefault();
        if (event.key === "Escape") input.setAttribute("aria-expanded", "false");
        if (event.key !== "F5") send(event.key);
    }
});
input.focus();
align();
