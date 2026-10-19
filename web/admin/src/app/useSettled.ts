import { useEffect, useState } from 'react';

/** A value that settles after `delay` without a change, so typing is one request, not one per key. */
export const useSettled = <T>(value: T, delay: number): T => {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => {
      setSettled(value);
    }, delay);
    return () => {
      clearTimeout(timer);
    };
  }, [value, delay]);
  return settled;
};
