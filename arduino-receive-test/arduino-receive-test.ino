#include <WiFi.h>
#include <lwip/sockets.h>
#include <lwip/netdb.h>
#include "octafont-regular.h"
#include "octafont-bold.h"
#include "freertos/task.h"

#define ANALOG_PIN 1

void callback() {
  Serial.println("done");  
}

// Result structure for ADC Continuous reading
adc_continuous_data_t *result = NULL;

volatile bool adc_coversion_done = false;
// ISR Function that will be triggered when ADC conversion is done
void ARDUINO_ISR_ATTR adcComplete() {
  analogContinuousRead(((adc_continuous_data_t **)&result), 0);
}

void setup()
{
  Serial.begin(115200);
  uint8_t pins[1] = {1};
  analogContinuousSetWidth(12);
  
analogContinuous(pins, 1, 1, 50000, &adcComplete);
analogContinuousSetAtten(ADC_ATTENDB_MAX);
  // Start ADC Continuous conversions
  analogContinuousStart();
  
}



void loop()
{
    // Check if conversion is done and try to read data
  if (result) {
    
      auto avg_raw = result[0].avg_read_raw;
      result = NULL;
      Serial.println(avg_raw);

  }
}
