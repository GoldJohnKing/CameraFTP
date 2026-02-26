import { ToggleSwitch } from './ui';

interface AutoStartToggleProps {
  enabled: boolean;
  isLoading: boolean;
  onToggle: () => Promise<void>;
}

export function AutoStartToggle({ enabled, isLoading, onToggle }: AutoStartToggleProps) {
  return (
    <ToggleSwitch
      enabled={enabled}
      onChange={onToggle}
      label="开机自启动"
      description="系统启动时自动运行图传伴侣"
      disabled={isLoading}
    />
  );
}
