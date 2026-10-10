// The studio mark: Ippon-datara, the one-legged, one-eyed forge yokai, flat,
// with its yellow tusks and its hammer: the shaft over its shoulder and the
// red-hot head on its back.
export function BrandMark({ size = 34 }: { size?: number }) {
  return (
    <svg
      className="brand-mark"
      viewBox="0 0 64 64"
      width={size}
      height={size}
      aria-hidden="true"
    >
      <rect width="64" height="64" rx="15" fill="#F4E9DA" />
      <path d="M8 17L47 27" stroke="#7A7E86" strokeWidth="3.4" strokeLinecap="round" />
      <rect x="41" y="18" width="18" height="17" rx="3" fill="#A8382F" stroke="#F4E9DA" strokeWidth="2" />
      <path d="M19.5 33h23l-3.5 13.5h-15.5z" fill="#4A3229" />
      <path d="M23 36l-4.5-14" stroke="#4A3229" strokeWidth="5.5" strokeLinecap="round" />
      <circle cx="18" cy="19.5" r="3.6" fill="#4A3229" />
      <path d="M42.5 36.5l4.5 9" stroke="#4A3229" strokeWidth="5.5" strokeLinecap="round" />
      <path d="M29 45v8" stroke="#4A3229" strokeWidth="9" strokeLinecap="round" />
      <path d="M22 52h14a3.25 3.25 0 0 1 0 6.5H22a3.25 3.25 0 0 1 0-6.5z" fill="#4A3229" />
      <g transform="translate(31 22) scale(.8) translate(-32 -24)">
        <path d="M16 31l-5-6 6-2-4-8 8 2V9l7 5 4-9 4 9 7-5v8l8-2-4 8 6 2-5 6c0 6-7 10-16 10s-16-4-16-10z" fill="#DA4A2C" />
        <circle cx="32" cy="27" r="9" fill="#18181B" />
        <circle cx="32" cy="25.5" r="5" fill="#FF6B3D" />
        <circle cx="32" cy="25.5" r="2" fill="#FFE7BF" />
        <path d="M27.5 34.5c-4.5-.3-6.8-3.2-6.6-7.6 1.6 2.8 3.6 4.3 6.9 4.4z" fill="#F5C445" />
        <path d="M36.5 34.5c4.5-.3 6.8-3.2 6.6-7.6-1.6 2.8-3.6 4.3-6.9 4.4z" fill="#F5C445" />
      </g>
    </svg>
  );
}
