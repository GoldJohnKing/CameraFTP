import type { PreviewWindowConfig } from './index';

/**
 * Event payload for preview configuration changes
 * Emitted when preview settings are updated
 */
export interface ConfigChangedEvent {
    config: PreviewWindowConfig;
}
