const previous = await dioxus.recv();
const picker = document.getElementById("oxdm-console-sidebar-content");
if (!picker) return;
const filter = picker.querySelector("#device-list-filter");
const cards = Array.from(picker.querySelectorAll("button.device-card"));
const index = cards.indexOf(document.activeElement);
const next = index < 0 ? (previous ? cards.length - 1 : 0) :
    index + (previous ? -1 : 1);
(cards[next] ?? filter)?.focus();
