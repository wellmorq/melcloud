export function formatTemperature(value) {
  if (value == null) return "--";
  return `${formatScaleLabel(value)}°C`;
}

export function formatScaleLabel(value) {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

export function formatWeatherDay(date, language) {
  if (!date) return "";
  const locale = language === "ru" ? "ru-RU" : "en-US";
  const label = new Intl.DateTimeFormat(locale, { weekday: "short" }).format(new Date(date));
  const cleaned = label.replace(".", "");
  return cleaned ? cleaned.charAt(0).toUpperCase() + cleaned.slice(1) : "";
}
