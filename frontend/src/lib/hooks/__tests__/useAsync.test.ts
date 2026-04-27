import { renderHook, act, waitFor } from '@testing-library/react';
import { useAsync } from '../useAsync';

describe('useAsync', () => {
  it('initializes with default state', () => {
    const mockFn = jest.fn();
    const { result } = renderHook(() => useAsync(mockFn));

    expect(result.current.data).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(typeof result.current.execute).toBe('function');
  });

  it('executes async function and updates state on success', async () => {
    const mockData = { test: 'data' };
    const mockFn = jest.fn().mockResolvedValue(mockData);
    const { result } = renderHook(() => useAsync(mockFn, { immediate: true }));

    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.data).toEqual(mockData);
    expect(result.current.error).toBeNull();
  });

  it('handles errors correctly', async () => {
    const mockError = new Error('Test error');
    const mockFn = jest.fn().mockRejectedValue(mockError);
    const { result } = renderHook(() => useAsync(mockFn, { immediate: true }));

    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.data).toBeNull();
    expect(result.current.error).toEqual(mockError);
  });

  it('allows manual execution', async () => {
    const mockData = { manual: 'execution' };
    const mockFn = jest.fn().mockResolvedValue(mockData);
    const { result } = renderHook(() => useAsync(mockFn));

    expect(result.current.loading).toBe(false);

    act(() => {
      result.current.execute();
    });

    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.data).toEqual(mockData);
  });
});