const shellTitle = "CreamShell";
const barHeight = 44;

function isCreamShell(window) {
    return window && window.caption === shellTitle;
}

function popupSize(window) {
    if (!window) return null;
    if (window.caption === "CreamShell Weather") return { width: 240, height: 112, offset: 54 };
    if (window.caption === "CreamShell Current Playing") return { width: 310, height: 128, offset: 172 };
    // Keep the clock popup attached to the clock/tray end of the dock.
    if (window.caption === "CreamShell Clock") return { width: 260, height: 156, anchor: "right" };
    if (window.caption === "CreamShell App Drawer") return { width: 620, height: 480, anchor: "center" };
    if (window.caption === "CreamShell Control Center") return { width: 360, height: 360, anchor: "right" };
    return null;
}

function placeWindow(window) {
    if (!window) return;
    const popup = popupSize(window);
    const area = workspace.virtualScreenGeometry;
    let width;
    let height;
    let x;
    let y;
    if (isCreamShell(window)) {
        width = area.width;
        height = barHeight;
        x = area.x;
        y = area.y + area.height - height;
        window.skipTaskbar = true;
        window.skipSwitcher = true;
    } else if (popup) {
        width = popup.width;
        height = popup.height;
        if (popup.anchor === "center") {
            x = area.x + (area.width - width) / 2;
        } else if (popup.anchor === "right") {
            x = area.x + area.width - width - 16;
        } else {
            x = area.x + popup.offset;
        }
        y = area.y + area.height - barHeight - height;
        window.skipTaskbar = true;
        window.skipSwitcher = true;
    } else {
        return;
    }
    if (!window.keepAbove) window.keepAbove = true;
    if (window.x !== x || window.y !== y || window.width !== width || window.height !== height) {
        window.frameGeometry = { x, y, width, height };
    }
}

function place() {
    workspace.stackingOrder.forEach(placeWindow);
}

function closeInactivePopups(active) {
    workspace.stackingOrder.forEach(function(window) {
        if (popupSize(window) && window !== active && !window.active) {
            window.closeWindow();
        }
    });
}

function track(window) {
    if (!window) return;
    window.captionChanged.connect(place);
}

function start(window) {
    track(window);
    place();
}

workspace.windowAdded.connect(start);
workspace.windowActivated.connect(function(window) {
    closeInactivePopups(window);
    place();
});
workspace.virtualScreenGeometryChanged.connect(place);
workspace.stackingOrder.forEach(track);
place();
