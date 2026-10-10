// The studio mark: the one-legged, one-eyed forge yokai (Ippon-datara),
// flat, with its yellow tusks and the hammer on its shoulder.
export function BrandMark({ size = 34 }: { size?: number }) {
  return (
    <svg
      className="brand-mark"
      viewBox="0 0 64 64"
      width={size}
      height={size}
      aria-hidden="true"
    >
      <rect width="64" height="64" rx="15" fill="#18181B" />
      <path d="M9 17L57 35" stroke="#8D939C" strokeWidth="4" strokeLinecap="round" />
      <rect x="3" y="10" width="11" height="15" rx="2.5" fill="#B9BFC7" transform="rotate(20 8.5 17.5)" />
      <path d="M24 41L15.5 22" stroke="#5A3B2E" strokeWidth="6.5" strokeLinecap="round" />
      <circle cx="15" cy="20.5" r="4" fill="#5A3B2E" />
      <path d="M21 38c-1 8 4 13 11 13s12-5 11-13z" fill="#5A3B2E" />
      <path d="M40 42l7 10" stroke="#5A3B2E" strokeWidth="6.5" strokeLinecap="round" />
      <path d="M32 49l-.5 7" stroke="#5A3B2E" strokeWidth="7.5" strokeLinecap="round" />
      <rect x="25" y="54.5" width="18" height="6.5" rx="3.25" fill="#5A3B2E" />
      <path d="M16.5 32.5l5-2.2M41.5 47.5l4-2.6M28.3 52.5h6.4" stroke="#F2A03A" strokeWidth="1.7" strokeLinecap="round" />
      <path d="M16 31l-5-6 6-2-4-8 8 2V9l7 5 4-9 4 9 7-5v8l8-2-4 8 6 2-5 6c0 6-7 10-16 10s-16-4-16-10z" fill="#DA4A2C" />
      <circle cx="32" cy="27" r="9" fill="#18181B" />
      <circle cx="32" cy="25.5" r="5" fill="#FF6B3D" />
      <circle cx="32" cy="25.5" r="2" fill="#FFE7BF" />
      <path d="M27.5 34.5c-4.5-.3-6.8-3.2-6.6-7.6 1.6 2.8 3.6 4.3 6.9 4.4z" fill="#F5C445" />
      <path d="M36.5 34.5c4.5-.3 6.8-3.2 6.6-7.6-1.6 2.8-3.6 4.3-6.9 4.4z" fill="#F5C445" />
    </svg>
  );
}
