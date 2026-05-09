/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useRef, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown } from 'lucide-react';

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  className?: string;
}

export function Select({ value, options, onChange, disabled, className = '' }: SelectProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({});

  const selected = options.find(o => o.value === value);

  const updatePosition = useCallback(() => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    setDropdownStyle({
      position: 'fixed',
      top: rect.bottom + 4,
      left: rect.left,
      width: rect.width,
      zIndex: 9999,
    });
  }, []);

  useEffect(() => {
    if (!open) return;
    updatePosition();

    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      const insideTrigger = containerRef.current?.contains(target);
      const insideDropdown = dropdownRef.current?.contains(target);
      if (!insideTrigger && !insideDropdown) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      window.removeEventListener('scroll', updatePosition, true);
      window.removeEventListener('resize', updatePosition);
    };
  }, [open, updatePosition]);

  return (
    <div ref={containerRef} className={className}>
      <button
        type="button"
        onClick={() => !disabled && setOpen(prev => !prev)}
        disabled={disabled}
        className={`
          w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-left
          bg-white text-gray-700 flex items-center justify-between gap-2
          focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent
          ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:border-gray-300'}
        `}
      >
        <span className="truncate">{selected?.label ?? value}</span>
        <ChevronDown className={`w-4 h-4 text-gray-400 shrink-0 transition-transform duration-200 ${open ? 'rotate-180' : ''}`} />
      </button>

      {open && createPortal(
        <div ref={dropdownRef} style={dropdownStyle} className="bg-white border border-gray-200 rounded-lg shadow-lg py-1 max-h-60 overflow-auto">
          {options.map(option => (
            <button
              key={option.value}
              type="button"
              onClick={() => {
                onChange(option.value);
                setOpen(false);
              }}
              className={`
                w-full px-3 py-2 text-left text-sm transition-colors
                ${option.value === value
                  ? 'bg-blue-50 text-blue-700 font-medium'
                  : 'text-gray-700 hover:bg-gray-50'
                }
              `}
            >
              {option.label}
            </button>
          ))}
        </div>,
        document.body,
      )}
    </div>
  );
}
