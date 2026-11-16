const handle = document.getElementById("oxdm-console-sidebar-resizer");
if (!handle) return;
const frame = handle.parentElement;
let drag;

const bounds = () => {
    const style = getComputedStyle(frame);
    return [parseFloat(style.minWidth), parseFloat(style.maxWidth)];
};
const width = () => Math.round(frame.getBoundingClientRect().width);
const updateValue = () => {
    const [min, max] = bounds();
    handle.setAttribute("aria-valuemin", min);
    handle.setAttribute("aria-valuemax", Math.floor(max));
    handle.setAttribute("aria-valuenow", width());
};
const resize = (next) => {
    const [min, max] = bounds();
    const clamped = Math.round(Math.max(min, Math.min(Math.floor(max), next)));
    frame.style.setProperty("--console-sidebar-width", clamped + "px");
    updateValue();
};

handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || drag) return;
    handle.focus({ preventScroll: true });
    handle.setPointerCapture(event.pointerId);
    drag = { id: event.pointerId, x: event.clientX, width: width() };
    handle.setAttribute("data-dragging", "");
    event.preventDefault();
});
handle.addEventListener("pointermove", (event) => {
    if (drag?.id === event.pointerId) resize(drag.width + event.clientX - drag.x);
});
handle.addEventListener("lostpointercapture", (event) => {
    if (drag?.id !== event.pointerId) return;
    drag = undefined;
    handle.removeAttribute("data-dragging");
});
handle.addEventListener("keydown", (event) => {
    const step = event.shiftKey ? 48 : 16;
    switch (event.key) {
        case "ArrowLeft": resize(width() - step); break;
        case "ArrowRight": resize(width() + step); break;
        case "Home": resize(bounds()[0]); break;
        case "End": resize(bounds()[1]); break;
        default: return;
    }
    event.preventDefault();
    event.stopPropagation();
});

updateValue();
const observer = new ResizeObserver(() => {
    if (handle.isConnected) updateValue();
    else observer.disconnect();
});
observer.observe(handle);
observer.observe(document.documentElement);
