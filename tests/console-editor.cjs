const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

async function test() {
    const listeners = new Map();
    const messages = [];
    const attrs = new Map([["aria-expanded", "false"], ["aria-activedescendant", "option"]]);
    const classes = new Set();
    const option = {
        id: "option",
        parentElement: { dataset: { source: "ev" } },
        dataset: { start: "0", end: "2", caret: "2", replacement: "events " },
    };
    const input = {
        value: "",
        scrollLeft: 0,
        classList: { add: (name) => classes.add(name), remove: (name) => classes.delete(name) },
        addEventListener: (event, listener) => listeners.set(event, listener),
        closest: () => ({ addEventListener() {} }),
        getAttribute: (name) => attrs.get(name),
        setAttribute: (name, value) => attrs.set(name, value),
        focus: () => listeners.get("focus")(),
        setSelectionRange(start, end) {
            assert.equal(start, end);
            this.selectionEnd = end;
        },
        setRangeText(value, start, end) {
            this.value = this.value.slice(0, start) + value + this.value.slice(end);
            this.selectionEnd = start + value.length;
        },
    };
    const paint = { style: {} };
    const elements = new Map([["oxdm-console-input", input], ["oxdm-console-paint", paint], ["option", option]]);
    const document = { getElementById: (id) => elements.get(id) };
    const dioxus = { recv: async () => "", send: (message) => messages.push({ ...message }) };
    const editor = fs.readFileSync(path.join(__dirname, "../assets/console-editor.js"), "utf8");
    await vm.runInNewContext("(async () => {" + editor + "})()", { document, dioxus });

    function type(text) {
        input.value += text;
        input.selectionEnd = input.value.length;
        listeners.get("input")();
    }
    function key(key) {
        const event = {
            key,
            stopPropagation() { this.stopped = true; },
            preventDefault() { this.prevented = true; },
        };
        listeners.get("keydown")(event);
        return event;
    }

    assert.equal(key("Tab").prevented, undefined);
    type("ev");
    attrs.set("aria-expanded", "true");
    assert.equal(key("Tab").prevented, true);
    type("wa");
    assert.equal(input.value, "events wa");
    assert.deepEqual(messages.at(-1), { action: "change", text: "events wa", caret: 9 });
    listeners.get("console-run")();
    assert.deepEqual(messages.at(-1), { action: "run", text: "events wa", caret: 9 });

    // The DOM can advance before Rust renders another candidate list.
    attrs.set("aria-expanded", "true");
    key("Tab");
    assert.equal(input.value, "events wa");

    assert.equal(key("Escape").stopped, true);
    assert.equal(messages.at(-1).action, "Escape");
    assert.equal(attrs.get("aria-expanded"), "false");
    const afterDismiss = messages.length;
    assert.equal(key("Escape").stopped, undefined);
    assert.equal(messages.length, afterDismiss);

    const before = messages.length;
    listeners.get("compositionstart")();
    type("鏡頭🙂");
    assert.equal(key("Enter").prevented, undefined);
    assert.equal(key("Escape").stopped, true);
    listeners.get("console-run")();
    assert.equal(messages.length, before);
    listeners.get("compositionend")();
    assert.equal(messages.at(-1).caret, 13);
    assert.equal(messages.at(-1).text, "events wa鏡頭🙂");

    let blocked = false;
    listeners.get("paste")({ clipboardData: { getData: () => "ev\nwatch" },
        preventDefault() { blocked = true; } });
    assert.equal(blocked, true);
    assert.equal(messages.at(-1).action, "multiline");
}

async function testPicker() {
    const pickerScript = fs.readFileSync(path.join(__dirname, "../assets/console-picker.js"), "utf8");
    const document = {};
    const focusable = () => ({ focus() { document.activeElement = this; } });
    const filter = focusable();
    const first = focusable();
    const second = focusable();
    let cards = [first, second];
    const picker = {
        querySelector: () => filter,
        querySelectorAll: () => cards,
    };
    document.getElementById = (id) => id === "oxdm-console-sidebar-content" ? picker : null;
    const move = (previous) => vm.runInNewContext("(async () => {" + pickerScript + "})()", {
        document, dioxus: { recv: async () => previous },
    });

    filter.focus();
    await move(false);
    assert.equal(document.activeElement, first);
    await move(false);
    assert.equal(document.activeElement, second);
    await move(false);
    assert.equal(document.activeElement, filter);
    await move(true);
    assert.equal(document.activeElement, second);
    cards = [];
    await move(true);
    assert.equal(document.activeElement, filter);
}

Promise.all([test(), testPicker()]).catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
