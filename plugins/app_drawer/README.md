# App drawer

The `app_drawer` panel presents the shared application catalog as a searchable
category grid. The `app_launcher` island opens it from either bar.

On desktop-entry platforms, right-click an application for **Pin**, or drag its
real `.desktop` file to the [Pinned apps](../pinned_apps/README.md) widget. Drags
use `text/uri-list` with the native Copy action. A normal click launches the app;
a drag does not launch it. Pinning errors stay visible in the drawer.

Application scanning, icon resolution, category mapping and launch metadata live
in `coconut-plugin-kit::applications`, so other plugins can present the same
catalog without depending on this panel. The shared pin file is accessed through
`coconut-plugin-kit::pins`, with no drawer-to-widget protocol.
