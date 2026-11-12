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
        const event = { key, stopPropagation() {}, preventDefault() { this.prevented = true; } };
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

    const before = messages.length;
    listeners.get("compositionstart")();
    type("鏡頭🙂");
    assert.equal(key("Enter").prevented, undefined);
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

test().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
