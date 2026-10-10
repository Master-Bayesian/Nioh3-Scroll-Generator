import { brandMark } from "./brand-mark";

// The studio mark: Ippon-datara, the forge yokai with its hammer.
export function BrandMark({ size = 40 }: { size?: number }) {
  return (
    <img
      className="brand-mark"
      src={brandMark}
      width={size}
      height={size}
      alt=""
      draggable={false}
    />
  );
}
