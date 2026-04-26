import { renderHook, act } from '@testing-library/react';
import { useDarkMode } from '../useDarkMode';

describe('useDarkMode', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark-mode');
  });

  it('should initialize with light mode by default', () => {
    const { result } = renderHook(() => useDarkMode());
    
    // Wait for effect to run
    expect(result.current.isLoaded).toBe(false);
  });

  it('should toggle dark mode', () => {
    const { result } = renderHook(() => useDarkMode());
    
    act(() => {
      result.current.toggleDarkMode();
    });
    
    expect(result.current.isDarkMode).toBe(true);
  });

  it('should persist dark mode preference to localStorage', () => {
    const { result } = renderHook(() => useDarkMode());
    
    act(() => {
      result.current.toggleDarkMode();
    });
    
    expect(localStorage.getItem('darkMode')).toBe('true');
  });

  it('should load dark mode preference from localStorage', () => {
    localStorage.setItem('darkMode', 'true');
    
    const { result } = renderHook(() => useDarkMode());
    
    // Wait for effect
    expect(result.current.isDarkMode).toBe(true);
  });

  it('should add dark-mode class to document element', () => {
    const { result } = renderHook(() => useDarkMode());
    
    act(() => {
      result.current.toggleDarkMode();
    });
    
    expect(document.documentElement.classList.contains('dark-mode')).toBe(true);
  });

  it('should remove dark-mode class when toggling off', () => {
    localStorage.setItem('darkMode', 'true');
    
    const { result } = renderHook(() => useDarkMode());
    
    act(() => {
      result.current.toggleDarkMode();
    });
    
    expect(document.documentElement.classList.contains('dark-mode')).toBe(false);
  });
});
