import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const POLL_INTERVAL_SECONDS = 1;

export default class OpenUsageAnchorExtension extends Extension {
    enable() {
        this._statusIcon = null;
        this._capturedEventId = 0;
        this._sourceId = GLib.timeout_add_seconds(
            GLib.PRIORITY_DEFAULT,
            POLL_INTERVAL_SECONDS,
            () => {
                this._refresh();
                return GLib.SOURCE_CONTINUE;
            });
        this._refresh();
    }

    disable() {
        if (this._sourceId) {
            GLib.Source.remove(this._sourceId);
            this._sourceId = 0;
        }
        this._detachStatusIcon();
    }

    _refresh() {
        const statusIcon = Object.values(Main.panel.statusArea).find(icon =>
            this._isOpenUsage(icon));
        if (statusIcon !== this._statusIcon) {
            this._detachStatusIcon();
            this._statusIcon = statusIcon ?? null;
            if (this._statusIcon) {
                this._capturedEventId = this._statusIcon.connect(
                    'captured-event',
                    (_actor, event) => this._onCapturedEvent(event));
            }
        }

        if (this._statusIcon)
            this._post('/v1/linux-panel/anchor');
    }

    _detachStatusIcon() {
        if (this._statusIcon && this._capturedEventId)
            this._statusIcon.disconnect(this._capturedEventId);
        this._statusIcon = null;
        this._capturedEventId = 0;
    }

    _isOpenUsage(icon) {
        const values = [
            icon?.uniqueId,
            icon?.get_accessible_name?.(),
            icon?.icon?.wm_class,
        ];
        return values.some(value => String(value ?? '').toLowerCase()
            .includes('openusage'));
    }

    _onCapturedEvent(event) {
        if (event.type() !== Clutter.EventType.BUTTON_RELEASE ||
            event.get_button() !== Clutter.BUTTON_PRIMARY)
            return Clutter.EVENT_PROPAGATE;

        this._post('/v1/linux-panel/open');
        return Clutter.EVENT_STOP;
    }

    _post(path) {
        const actor = this._statusIcon?._box ?? this._statusIcon;
        if (!actor)
            return;

        const [x, y] = actor.get_transformed_position();
        const [width, height] = actor.get_transformed_size();
        if (![x, y, width, height].every(Number.isFinite))
            return;

        const body = JSON.stringify({
            centerX: x + width / 2,
            bottomY: y + height,
        });
        const request = [
            `POST ${path} HTTP/1.1`,
            'Host: 127.0.0.1:6736',
            'Content-Type: application/json',
            `Content-Length: ${body.length}`,
            'Connection: close',
            '',
            body,
        ].join('\r\n');

        const client = new Gio.SocketClient();
        client.connect_to_host_async('127.0.0.1', 6736, null, (_client, result) => {
            try {
                const connection = client.connect_to_host_finish(result);
                const output = new Gio.DataOutputStream({
                    base_stream: connection.output_stream,
                });
                output.put_string(request, null);
                output.close(null);
                connection.close(null);
            } catch (error) {
                console.warn(`OpenUsage anchor request failed: ${error}`);
            }
        });
    }
}
