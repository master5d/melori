import React from "react";

const EchoIcon = ({
  width,
  height,
  className,
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 1024 1024"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <rect
        x="256"
        y="384"
        width="128"
        height="256"
        rx="64"
        fill="currentColor"
      />
      <rect
        x="448"
        y="256"
        width="128"
        height="512"
        rx="64"
        fill="currentColor"
      />
      <rect
        x="640"
        y="384"
        width="128"
        height="256"
        rx="64"
        fill="currentColor"
      />
    </svg>
  );
};

export default EchoIcon;
