/** The value a label names in a block of label and value rows (`<dt>`, `<dd>`). */
import { screen } from '@testing-library/react';

const valueAfter = (term: HTMLElement, label: string): HTMLElement => {
  const value = term.nextElementSibling;
  if (!(value instanceof HTMLElement) || value.tagName !== 'DD') {
    throw new Error(`no value after ${label}`);
  }
  return value;
};

export const fact = (label: string): HTMLElement => valueAfter(screen.getByText(label, { selector: 'dt' }), label);

export const findFact = async (label: string): Promise<HTMLElement> =>
  valueAfter(await screen.findByText(label, { selector: 'dt' }, { timeout: 5000 }), label);
