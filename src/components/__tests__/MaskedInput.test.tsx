/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState } from 'react';
import { fireEvent, render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MaskedInput } from '../ui/MaskedInput';

const DOT = '\u2022';

/**
 * Controlled harness that mirrors real usage: the parent stores the
 * (unmasked) onChange payload and feeds it back as `value`.
 */
function ControlledMaskedInput({ visible, initialValue }: { visible: boolean; initialValue: string }) {
  const [value, setValue] = useState(initialValue);
  return (
    <MaskedInput
      visible={visible}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      data-testid="masked"
    />
  );
}

function setup({ visible = false, value = 'abcd' }: { visible?: boolean; value?: string } = {}) {
  const onChange = vi.fn();
  const utils = render(
    <MaskedInput visible={visible} value={value} onChange={onChange} data-testid="masked" />,
  );
  const input = utils.getByTestId('masked') as HTMLInputElement;
  return { input, onChange, ...utils };
}

/** Simulate a DOM change event on the masked input with an explicit caret. */
function editTo(input: HTMLInputElement, raw: string, selectionStart?: number) {
  fireEvent.change(input, {
    target: selectionStart === undefined
      ? { value: raw }
      : { value: raw, selectionStart, selectionEnd: selectionStart },
  });
}

function lastPayload(onChange: ReturnType<typeof vi.fn>): string {
  expect(onChange).toHaveBeenCalled();
  const event = onChange.mock.calls[onChange.mock.calls.length - 1][0] as React.ChangeEvent<HTMLInputElement>;
  return event.target.value;
}

describe('MaskedInput rendering', () => {
  it('renders the real value when visible', () => {
    const { input } = setup({ visible: true, value: 'secret' });
    expect(input.value).toBe('secret');
  });

  it('renders one mask dot per character when hidden', () => {
    const { input } = setup({ visible: false, value: 'abcd' });
    expect(input.value).toBe(`${DOT}${DOT}${DOT}${DOT}`);
  });

  it('renders an empty field for empty or missing values', () => {
    const { input } = setup({ value: '' });
    expect(input.value).toBe('');

    // Scoped query: RTL queries are bound to document.body, so a second
    // render in the same test would find both inputs.
    const omitted = render(<MaskedInput visible={false} onChange={vi.fn()} />);
    const omittedInput = omitted.container.querySelector('input') as HTMLInputElement;
    expect(omittedInput.value).toBe('');
  });

  it('always uses type="text" to avoid the Android secure keyboard', () => {
    const { input } = setup({ visible: false, value: 'abcd' });
    expect(input.type).toBe('text');
  });

  it('reflects controlled value changes coming from outside', () => {
    const onChange = vi.fn();
    const { getByTestId, rerender } = render(
      <MaskedInput visible={false} value="abcd" onChange={onChange} data-testid="masked" />,
    );
    const input = getByTestId('masked') as HTMLInputElement;

    rerender(<MaskedInput visible={false} value="longer" onChange={onChange} data-testid="masked" />);
    expect(input.value).toBe(DOT.repeat(6));

    rerender(<MaskedInput visible={true} value="longer" onChange={onChange} data-testid="masked" />);
    expect(input.value).toBe('longer');
  });
});

describe('MaskedInput visible editing', () => {
  it('passes the raw change event through untouched', () => {
    // Capture target.value synchronously inside the handler: React restores
    // controlled input values after the event, so reading later is unsafe.
    const seen: string[] = [];
    const onChange = vi.fn((e: React.ChangeEvent<HTMLInputElement>) => {
      seen.push(e.target.value);
    });
    const { getByTestId } = render(
      <MaskedInput visible={true} value="ab" onChange={onChange} data-testid="masked" />,
    );
    fireEvent.change(getByTestId('masked'), { target: { value: 'abc' } });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(seen).toEqual(['abc']);
  });
});

describe('MaskedInput masked editing — insertion', () => {
  it('appends a character typed at the end', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, `${DOT}${DOT}${DOT}${DOT}X`);
    expect(lastPayload(onChange)).toBe('abcdX');
  });

  it('prepends a character typed at the start', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, `X${DOT}${DOT}${DOT}${DOT}`);
    expect(lastPayload(onChange)).toBe('Xabcd');
  });

  it('inserts a character in the middle of the value', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, `${DOT}${DOT}X${DOT}${DOT}`);
    expect(lastPayload(onChange)).toBe('abXcd');
  });

  it('inserts multiple characters in the middle of the value', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, `${DOT}${DOT}XY${DOT}${DOT}`);
    expect(lastPayload(onChange)).toBe('abXYcd');
  });

  it('replaces the whole value when a full unmasked value is pasted over a selection', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, 'wxyz');
    expect(lastPayload(onChange)).toBe('wxyz');
  });

  it('replaces the whole value on select-all followed by typing one character', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, 'Z');
    expect(lastPayload(onChange)).toBe('Z');
  });

  it('replaces the covered character when a partial value is pasted mid-string', () => {
    // The second dot was selected and replaced by the pasted "XY".
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, `${DOT}XY${DOT}${DOT}`);
    expect(lastPayload(onChange)).toBe('aXYcd');
  });

  it('builds a value from scratch when the field starts empty', () => {
    const { input, onChange } = setup({ value: '' });
    editTo(input, 'a');
    expect(lastPayload(onChange)).toBe('a');
  });
});

describe('MaskedInput masked editing — deletion', () => {
  it('removes the last character on backspace at the end', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, DOT.repeat(3), 3);
    expect(lastPayload(onChange)).toBe('abc');
  });

  it('removes the character before the caret on backspace mid-string', () => {
    // Caret was after the 2nd dot; backspace deletes the 2nd character ("b").
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, DOT.repeat(3), 1);
    expect(lastPayload(onChange)).toBe('acd');
  });

  it('removes the character at the caret on Delete mid-string', () => {
    // Caret on the 3rd dot; Delete removes the 3rd character ("c").
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, DOT.repeat(3), 2);
    expect(lastPayload(onChange)).toBe('abd');
  });

  it('removes the first character on Delete at the start', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, DOT.repeat(3), 0);
    expect(lastPayload(onChange)).toBe('bcd');
  });

  it('removes a multi-character selection in the middle', () => {
    // "bc" was selected and deleted; caret lands where the selection started.
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, DOT.repeat(2), 1);
    expect(lastPayload(onChange)).toBe('ad');
  });

  it('clears the value on select-all followed by delete', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, '', 0);
    expect(lastPayload(onChange)).toBe('');
  });
});

describe('MaskedInput masked editing — no-op', () => {
  it('does not fire onChange when the masked value is unchanged', () => {
    const { input, onChange } = setup({ value: 'abcd' });
    editTo(input, DOT.repeat(4));
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe('MaskedInput controlled round-trip', () => {
  it('keeps the dot display in sync after an edit round-trip', () => {
    const { getByTestId } = render(<ControlledMaskedInput visible={false} initialValue="secret" />);
    const input = getByTestId('masked') as HTMLInputElement;
    expect(input.value).toBe(DOT.repeat(6));

    // Append "X" to the masked value: 6 dots + X
    editTo(input, `${DOT.repeat(6)}X`);
    expect(input.value).toBe(DOT.repeat(7));

    // Delete one character again
    editTo(input, DOT.repeat(6), 6);
    expect(input.value).toBe(DOT.repeat(6));
  });

  it('forwards its ref to the underlying input element', () => {
    const ref = vi.fn();
    render(<MaskedInput ref={ref} visible={false} value="ab" onChange={vi.fn()} />);
    expect(ref).toHaveBeenCalledTimes(1);
    expect(ref.mock.calls[0][0]).toBeInstanceOf(HTMLInputElement);
  });
});
