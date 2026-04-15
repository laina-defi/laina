/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
  theme: {
    fontFamily: {
      sans: ['Manrope', 'system-ui', 'sans-serif'],
    },
    colors: {
      white: '#fff',
      grey: {
        dark: '#555',
        DEFAULT: '#999',
        light: '#ebecf0',
        lighter: '#efefef',
      },
      black: '#000',
      primary: '#000',
      red: '#e51414',
      error: '#e51414',
      yellow: '#e18c02',
      warning: '#e18c02',
      blue: '#0048ff',
      info: '#0048ff',
      green: '#0fdd13',
      success: '#0fdd13',
      cyan: {
        DEFAULT: '#00D9FF',
        dark: '#00a8cc',
        light: '#e0f9ff',
      },
      magenta: {
        DEFAULT: '#FF0099',
        dark: '#cc007a',
        light: '#ffe0f5',
      },
    },
    fontSize: {
      sm: ['14px', '20px'],
      base: ['18px', '24px'],
      lg: ['20px', '28px'],
      xl: ['26px', '32px'],
      '2xl': ['32px', '40px'],
      '3xl': ['48px', '56px'],
    },
    borderRadius: {
      none: '0',
      DEFAULT: '16px',
      full: '9999px',
    },
    boxShadow: {
      DEFAULT: '-15px 15px 60px 15px rgba(0, 0, 0, 25%)',
    },
    extend: {
      spacing: {
        128: '32rem',
        160: '40rem',
        256: '64rem',
      },
    },
  },
  daisyui: {
    themes: [
      {
        light: {
          ...require('daisyui/src/theming/themes').light,
          neutral: '#000',
          '.btn-neutral:hover': {
            'background-color': '#555',
            'border-color': '#555',
          },
          primary: '#fff',
          '.btn-primary:hover': {
            'background-color': '#aaa',
            'border-color': '#aaa',
          },
        },
      },
    ],
  },
  plugins: [require('daisyui')],
};
