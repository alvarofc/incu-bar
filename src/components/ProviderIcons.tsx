// Provider brand icons as React components
// Icons sourced from Simple Icons (https://simpleicons.org)

import type { ProviderId } from '../lib/types';

interface IconProps {
  className?: string;
  'aria-hidden'?: boolean | 'true' | 'false';
}

// Claude (Anthropic) - Official logo from Simple Icons
export function ClaudeIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"/>
    </svg>
  );
}

// OpenAI (Codex) - Official logo from Simple Icons
export function CodexIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.855-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08-4.778 2.758a.795.795 0 0 0-.393.681zm1.097-2.365 2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z"/>
    </svg>
  );
}

// Cursor IDE - Official logo from cursor.com
export function CursorIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M19.144 5.633 10.506.646a1.708 1.708 0 0 0-1.712 0L.152 5.633A1.748 1.748 0 0 0 0 7.103v9.792c0 .523.275 1.008.72 1.27l8.642 4.863c.53.311 1.185.311 1.715 0l8.643-4.862c.446-.263.721-.748.721-1.27V7.103a1.747 1.747 0 0 0-.296-1.47ZM2.212 6.34h15.94c.453 0 .737.5.51.898L10.298 21.26c-.108.188-.39.112-.39-.106V12.86a.904.904 0 0 0-.455-.784L1.8 7.284c-.186-.108-.11-.944.412-.944Z"/>
    </svg>
  );
}

// GitHub Copilot - Official logo from Simple Icons
export function CopilotIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M23.922 16.997C23.061 18.492 18.063 22.02 12 22.02 5.937 22.02.939 18.492.078 16.997A.641.641 0 0 1 0 16.741v-2.869a.883.883 0 0 1 .053-.22c.372-.935 1.347-2.292 2.605-2.656.167-.429.414-1.055.644-1.517a10.098 10.098 0 0 1-.052-1.086c0-1.331.282-2.499 1.132-3.368.397-.406.89-.717 1.474-.952C7.255 2.937 9.248 1.98 11.978 1.98c2.731 0 4.767.957 6.166 2.093.584.235 1.077.546 1.474.952.85.869 1.132 2.037 1.132 3.368 0 .368-.014.733-.052 1.086.23.462.477 1.088.644 1.517 1.258.364 2.233 1.721 2.605 2.656a.841.841 0 0 1 .053.22v2.869a.641.641 0 0 1-.078.256Zm-11.75-5.992h-.344a4.359 4.359 0 0 1-.355.508c-.77.947-1.918 1.492-3.508 1.492-1.725 0-2.989-.359-3.782-1.259a2.137 2.137 0 0 1-.085-.104L4 11.746v6.585c1.435.779 4.514 2.179 8 2.179 3.486 0 6.565-1.4 8-2.179v-6.585l-.098-.104s-.033.045-.085.104c-.793.9-2.057 1.259-3.782 1.259-1.59 0-2.738-.545-3.508-1.492a4.359 4.359 0 0 1-.355-.508Zm2.328 3.25c.549 0 1 .451 1 1v2c0 .549-.451 1-1 1-.549 0-1-.451-1-1v-2c0-.549.451-1 1-1Zm-5 0c.549 0 1 .451 1 1v2c0 .549-.451 1-1 1-.549 0-1-.451-1-1v-2c0-.549.451-1 1-1Zm3.313-6.185c.136 1.057.403 1.913.878 2.497.442.544 1.134.938 2.344.938 1.573 0 2.292-.337 2.657-.751.384-.435.558-1.15.558-2.361 0-1.14-.243-1.847-.705-2.319-.477-.488-1.319-.862-2.824-1.025-1.487-.161-2.192.138-2.533.529-.269.307-.437.808-.438 1.578v.021c0 .265.021.562.063.893Zm-1.626 0c.042-.331.063-.628.063-.894v-.02c-.001-.77-.169-1.271-.438-1.578-.341-.391-1.046-.69-2.533-.529-1.505.163-2.347.537-2.824 1.025-.462.472-.705 1.179-.705 2.319 0 1.211.175 1.926.558 2.361.365.414 1.084.751 2.657.751 1.21 0 1.902-.394 2.344-.938.475-.584.742-1.44.878-2.497Z"/>
    </svg>
  );
}

// Google Gemini - Official logo from Simple Icons
export function GeminiIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M11.04 19.32Q12 21.51 12 24q0-2.49.93-4.68.96-2.19 2.58-3.81t3.81-2.55Q21.51 12 24 12q-2.49 0-4.68-.93a12.3 12.3 0 0 1-3.81-2.58 12.3 12.3 0 0 1-2.58-3.81Q12 2.49 12 0q0 2.49-.96 4.68-.93 2.19-2.55 3.81a12.3 12.3 0 0 1-3.81 2.58Q2.49 12 0 12q2.49 0 4.68.96 2.19.93 3.81 2.55t2.55 3.81"/>
    </svg>
  );
}

// JetBrains - Official logo from Simple Icons
export function JetbrainsIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M2.345 23.997A2.347 2.347 0 0 1 0 21.652V10.988C0 9.665.535 8.37 1.473 7.433l5.965-5.961A5.01 5.01 0 0 1 10.989 0h10.666A2.347 2.347 0 0 1 24 2.345v10.664a5.056 5.056 0 0 1-1.473 3.554l-5.965 5.965A5.017 5.017 0 0 1 13.007 24v-.003H2.345Zm8.969-6.854H5.486v1.371h5.828v-1.371ZM3.963 6.514h13.523v13.519l4.257-4.257a3.936 3.936 0 0 0 1.146-2.767V2.345c0-.678-.552-1.234-1.234-1.234H10.989a3.897 3.897 0 0 0-2.767 1.145L3.963 6.514Zm-.192.192L2.256 8.22a3.944 3.944 0 0 0-1.145 2.768v10.664c0 .678.552 1.234 1.234 1.234h10.666a3.9 3.9 0 0 0 2.767-1.146l1.512-1.511H3.771V6.706Z"/>
    </svg>
  );
}

// Google Cloud (Vertex AI) - Official logo from Simple Icons
export function VertexIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M12.19 1.5c-.6-.3-1.28-.3-1.88 0L3.23 5.49c-.63.32-1.11.89-1.32 1.58L.06 13.49c-.2.65-.1 1.35.27 1.92l4.42 6.88c.4.6 1.04 1.01 1.76 1.12l7.56 1.05c.68.1 1.37-.1 1.91-.53l5.46-4.42c.52-.43.86-1.03.96-1.7l1.05-7.56c.1-.72-.12-1.44-.6-1.98L17.4 2.82a2.413 2.413 0 0 0-1.79-.82l-3.42.5Zm3.92 6.7-3.12 5.4-3.12-5.4h6.24Zm-7.22 0 3.11 5.4-6.23-.01 3.12-5.39Zm7.22 10.8h-6.23l3.11-5.4 3.12 5.4Zm-7.22 0L5.77 13.6l6.23.01-3.11 5.39Z"/>
    </svg>
  );
}

// AWS (Kiro) - Terminal icon (no official Kiro icon yet)
export function KiroIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M7.39 16.539a8.262 8.262 0 0 1-1.574-.128 4.14 4.14 0 0 1-1.18-.387 1.846 1.846 0 0 1-.732-.622 1.481 1.481 0 0 1-.247-.846v-.625c0-.065.022-.12.066-.166a.218.218 0 0 1 .166-.069h.924c.065 0 .12.023.166.069a.223.223 0 0 1 .069.166v.359c0 .286.229.532.688.738.458.206 1.065.309 1.82.309.702 0 1.259-.113 1.67-.34.412-.227.618-.522.618-.884 0-.26-.109-.476-.327-.648-.219-.172-.584-.33-1.096-.473l-1.786-.492a5.969 5.969 0 0 1-1.052-.382 2.97 2.97 0 0 1-.785-.527 2.067 2.067 0 0 1-.49-.705 2.368 2.368 0 0 1-.168-.916c0-.636.264-1.148.793-1.535.528-.387 1.239-.653 2.133-.797A11.62 11.62 0 0 1 9.32 7.48c.57.04 1.118.113 1.645.218.528.104.98.25 1.356.436.377.187.675.414.894.684.218.27.328.585.328.944v.578c0 .065-.023.12-.07.166a.223.223 0 0 1-.165.069h-.924a.227.227 0 0 1-.166-.069.223.223 0 0 1-.069-.166v-.312c0-.143-.053-.275-.16-.398a1.244 1.244 0 0 0-.449-.32 3.466 3.466 0 0 0-.705-.231 6.96 6.96 0 0 0-.93-.128 9.8 9.8 0 0 0-1.098-.046c-.605 0-1.11.084-1.516.25-.406.167-.609.396-.609.689 0 .247.12.455.359.625.24.17.65.328 1.23.473l1.786.485c.423.117.811.26 1.163.43.352.169.656.369.91.601.254.232.453.5.596.804.143.305.214.653.214 1.046 0 .67-.27 1.21-.808 1.619-.54.41-1.264.69-2.172.845-.455.078-.944.117-1.468.117Zm8.216 0c-.54 0-1.035-.032-1.488-.097a5.006 5.006 0 0 1-1.189-.297 2.273 2.273 0 0 1-.794-.524 1.085 1.085 0 0 1-.29-.763v-.625c0-.065.022-.12.066-.166a.218.218 0 0 1 .166-.069h.924c.065 0 .12.023.166.069a.223.223 0 0 1 .069.166v.312c0 .247.218.463.653.648.436.186 1.036.278 1.801.278.69 0 1.233-.092 1.628-.275.396-.184.593-.431.593-.741 0-.26-.127-.469-.381-.625-.254-.156-.66-.302-1.217-.437l-1.74-.453c-.878-.228-1.544-.54-1.997-.936-.454-.397-.68-.904-.68-1.524 0-.345.08-.659.24-.94.161-.282.388-.525.682-.73a3.448 3.448 0 0 1 1.059-.48c.41-.114.863-.19 1.356-.227a12.24 12.24 0 0 1 2.828.078c.488.078.93.197 1.325.36.394.16.71.37.946.625.236.254.354.55.354.885v.547c0 .065-.023.12-.07.166a.223.223 0 0 1-.165.069h-.924a.227.227 0 0 1-.166-.069.223.223 0 0 1-.069-.166v-.266c0-.221-.2-.41-.601-.57-.4-.159-.946-.238-1.635-.238-.65 0-1.16.08-1.529.24-.37.16-.554.373-.554.64 0 .248.114.447.343.598.23.15.6.29 1.114.417l1.74.454c.92.24 1.61.557 2.071.952.462.396.693.894.693 1.496 0 .371-.09.701-.268.99-.18.29-.428.536-.746.74a3.738 3.738 0 0 1-1.124.464 6.61 6.61 0 0 1-1.392.193c-.26.013-.513.02-.76.02Z"/>
    </svg>
  );
}

// OpenCode - Terminal/CLI icon
export function OpencodeIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M20 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2m0 14H4V8h16m-2-6H6v2h12M7.5 17l1.41-1.41L6.33 13l2.58-2.59L7.5 9l-4 4m9 4 4-4-4-4-1.41 1.41L17.67 13l-2.58 2.59"/>
    </svg>
  );
}

// Factory (The Factory) - Factory building icon
export function FactoryIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M22 22H2V10l7 4V10l7 4 6-4v12ZM6 2h4v6H6V2Z"/>
    </svg>
  );
}

// Sourcegraph Amp - Official logo (simplified)
export function AmpIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M9.586 20.414a2 2 0 0 1 0-2.828l8.586-8.586H15a2 2 0 1 1 0-4h7a2 2 0 0 1 2 2v7a2 2 0 1 1-4 0v-3.172l-8.586 8.586a2 2 0 0 1-2.828 0ZM2 20a2 2 0 0 0 2 2h7a2 2 0 1 0 0-4H7.414l4.293-4.293a2 2 0 0 0-2.828-2.828L4.586 15.172V12a2 2 0 1 0-4 0v6c0 1.104.896 2 2 2Z"/>
    </svg>
  );
}

// Augment - Plus in circle icon
export function AugmentIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2Zm5 11h-4v4h-2v-4H7v-2h4V7h2v4h4v2Z"/>
    </svg>
  );
}

// z.ai - Lightning bolt icon
export function ZaiIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M7 2v11h3v9l7-12h-4l4-8H7Z"/>
    </svg>
  );
}

// MiniMax - Fullscreen/minimize icon
export function MinimaxIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M5 16h3v3h2v-5H5v2Zm3-8H5v2h5V5H8v3Zm6 11h2v-3h3v-2h-5v5Zm2-11V5h-2v5h5V8h-3Z"/>
    </svg>
  );
}

// Kimi (Moonshot AI) - Moon icon
export function KimiIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.389 5.389 0 0 1-4.4 2.26 5.403 5.403 0 0 1-3.14-9.8c-.44-.06-.9-.1-1.36-.1Z"/>
    </svg>
  );
}

// Antigravity - Rocket icon
export function AntigravityIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M13.13 22.19-1.63-3.83c1.57-.58 3.04-1.36 4.4-2.27l-2.55-.42 2.12-2.13-.65-3.64a21.076 21.076 0 0 0 6.18-6.18l3.64.65 2.13-2.12.42 2.55a15.44 15.44 0 0 0 2.27-4.4l3.83 1.63c-.27.68-.57 1.35-.89 2.01L22 6.93l-1.06-1.06-1.6 1.6L18 10l-2-2-.88-.89-1.3 1.3 2.59 2.59c-1.27.85-2.67 1.58-4.16 2.15l.43 2.42-2.12 2.13-2.55-.42c.91 1.36 1.69 2.83 2.27 4.4l2.86-1.23a17.67 17.67 0 0 1-2.01 1.08Zm2.68-17.2c.8.8 2.11.8 2.91 0s.8-2.1 0-2.9-2.1-.8-2.9 0-.81 2.1-.01 2.9Z"/>
    </svg>
  );
}

// Synthetic - Sparkle/star icon
export function SyntheticIcon({ className, 'aria-hidden': ariaHidden = true }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden={ariaHidden}>
      <path d="M12 2L9.19 8.63 2 9.24l5.46 4.73L5.82 21 12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2Z"/>
    </svg>
  );
}

// Map provider IDs to their icon components
export const ProviderIconMap: Record<ProviderId, React.ComponentType<IconProps>> = {
  claude: ClaudeIcon,
  codex: CodexIcon,
  cursor: CursorIcon,
  copilot: CopilotIcon,
  gemini: GeminiIcon,
  jetbrains: JetbrainsIcon,
  vertex: VertexIcon,
  kiro: KiroIcon,
  opencode: OpencodeIcon,
  factory: FactoryIcon,
  amp: AmpIcon,
  augment: AugmentIcon,
  zai: ZaiIcon,
  minimax: MinimaxIcon,
  kimi: KimiIcon,
  kimi_k2: KimiIcon,
  antigravity: AntigravityIcon,
  synthetic: SyntheticIcon,
};

// Helper component that renders the appropriate icon for a provider
export function ProviderIcon({ providerId, className }: { providerId: ProviderId; className?: string }) {
  const IconComponent = ProviderIconMap[providerId];
  if (!IconComponent) {
    return null;
  }
  return <IconComponent className={className} />;
}
