/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { forwardRef, useCallback } from 'react';

const MASK_CHAR = '\u2022'; // •
const MASK_RE = /\u2022/g;

/**
 * A text input that displays masked dots (•) when `visible` is false.
 *
 * Uses JS-level masking instead of `type="password"` or `-webkit-text-security`
 * to avoid triggering Android's secure keyboard and layout shift issues.
 *
 * `onChange` always receives the real (unmasked) value via `e.target.value`.
 */
export const MaskedInput = forwardRef<HTMLInputElement, MaskedInputProps>(
  function MaskedInput({ visible, value, onChange, ...rest }, ref) {
    const strValue = String(value ?? '');
    const displayValue = visible ? strValue : MASK_CHAR.repeat(strValue.length);

    const handleChange = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        if (visible) {
          onChange?.(e);
          return;
        }

        const raw = e.target.value;
        const nonDots = raw.replace(MASK_RE, '');

        let newValue: string;

        if (nonDots.length > 0) {
          if (raw.length !== nonDots.length) {
            // Insertion at a specific position (dots still present around new chars)
            const firstNew = raw.indexOf(nonDots[0]);
            const dotsAfter = raw.length - firstNew - nonDots.length;
            newValue = strValue.slice(0, firstNew) + nonDots + strValue.slice(strValue.length - dotsAfter);
          } else {
            // Full replacement (paste / select-all + type)
            newValue = nonDots;
          }
        } else if (raw.length < strValue.length) {
          // Deletion
          const cursor = e.target.selectionStart ?? raw.length;
          const deleted = strValue.length - raw.length;
          newValue = strValue.slice(0, cursor) + strValue.slice(cursor + deleted);
        } else {
          return; // No change
        }

        onChange?.({
          ...e,
          target: { ...e.target, value: newValue },
        } as React.ChangeEvent<HTMLInputElement>);
      },
      [visible, strValue, onChange]
    );

    return (
      <input
        ref={ref}
        type="text"
        autoComplete="off"
        value={displayValue}
        onChange={handleChange}
        {...rest}
      />
    );
  }
);

MaskedInput.displayName = 'MaskedInput';

type MaskedInputProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type'> & {
  /** Whether to show the real value. When false, displays • characters. */
  visible: boolean;
};
