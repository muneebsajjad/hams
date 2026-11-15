// A small set of 24-unit stroke icons, drawn here so the app ships no icon library.
import type { ReactNode } from "react";

function icon(children: ReactNode, filled = false) {
  return function Icon({ size = 16 }: { size?: number }) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill={filled ? "currentColor" : "none"}
        stroke={filled ? "none" : "currentColor"}
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {children}
      </svg>
    );
  };
}

export const MicIcon = icon(
  <>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
  </>,
);
export const BookIcon = icon(
  <>
    <path d="M5 4.5A1.5 1.5 0 0 1 6.5 3H19v15H6.5A1.5 1.5 0 0 0 5 19.5z" />
    <path d="M5 19.5A1.5 1.5 0 0 0 6.5 21H19M9 8h6" />
  </>,
);
export const ScissorsIcon = icon(
  <>
    <circle cx="6" cy="6.5" r="2.5" />
    <circle cx="6" cy="17.5" r="2.5" />
    <path d="M8.2 7.8 20 18M8.2 16.2 20 6" />
  </>,
);
export const NoteIcon = icon(
  <>
    <rect x="4.5" y="3" width="15" height="18" rx="2" />
    <path d="M8.5 8h7M8.5 12h7M8.5 16h4" />
  </>,
);
export const GearIcon = icon(
  <>
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.5 5.5l1.7 1.7M16.8 16.8l1.7 1.7M5.5 18.5l1.7-1.7M16.8 7.2l1.7-1.7" />
  </>,
);
export const ChartIcon = icon(<path d="M5 20V11M12 20V4M19 20v-6" />);
export const TrendIcon = icon(<path d="m4 16 5.5-5.5 3.5 3.5L20 7M14.5 7H20v5.5" />);
export const SearchIcon = icon(
  <>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m16 16 4.5 4.5" />
  </>,
);
export const WaveIcon = icon(<path d="M4 10v4M8 6v12M12 9v6M16 4v16M20 10v4" />);
export const CopyIcon = icon(
  <>
    <rect x="8.5" y="8.5" width="12" height="12" rx="2" />
    <path d="M15.5 8.5v-3a2 2 0 0 0-2-2h-8a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h3" />
  </>,
);
export const TrashIcon = icon(
  <path d="M4 6.5h16M9.5 6.5V4h5v2.5M6.5 6.5l.8 13a1.5 1.5 0 0 0 1.5 1.5h6.4a1.5 1.5 0 0 0 1.5-1.5l.8-13M10 11v6M14 11v6" />,
);
export const AlertIcon = icon(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7.5v5.5M12 16.5v.1" />
  </>,
);
export const PlayIcon = icon(<path d="M7 4.5v15a.6.6 0 0 0 .9.5l12-7.5a.6.6 0 0 0 0-1l-12-7.5a.6.6 0 0 0-.9.5z" />, true);
export const StopIcon = icon(<rect x="5.5" y="5.5" width="13" height="13" rx="2" />, true);
export const MicFillIcon = icon(
  <path d="M12 2.5a3.5 3.5 0 0 0-3.5 3.5v5a3.5 3.5 0 0 0 7 0V6A3.5 3.5 0 0 0 12 2.5zM5.5 10.2a.9.9 0 0 1 .9.9 5.6 5.6 0 0 0 11.2 0 .9.9 0 0 1 1.8 0 7.4 7.4 0 0 1-6.5 7.3V21a.9.9 0 0 1-1.8 0v-2.6a7.4 7.4 0 0 1-6.5-7.3.9.9 0 0 1 .9-.9z" />,
  true,
);
export const CheckIcon = icon(
  <path d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19zm4.6 7.3-5.2 5.6a.9.9 0 0 1-1.3 0l-2.6-2.6a.9.9 0 0 1 1.3-1.3l1.9 1.9 4.6-4.9a.9.9 0 0 1 1.3 1.3z" />,
  true,
);
export const CrossIcon = icon(
  <path d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19zm3.6 11.8a.9.9 0 0 1-1.3 1.3L12 13.3l-2.3 2.3a.9.9 0 0 1-1.3-1.3l2.3-2.3-2.3-2.3a.9.9 0 0 1 1.3-1.3l2.3 2.3 2.3-2.3a.9.9 0 0 1 1.3 1.3L13.3 12z" />,
  true,
);
