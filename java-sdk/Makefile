.PHONY: build clean

BUILD_DIR = build
CLASSES_DIR = $(BUILD_DIR)/classes
JAR = $(BUILD_DIR)/conch-plugin-sdk.jar

build:
	mkdir -p $(CLASSES_DIR)
	javac -d $(CLASSES_DIR) src/conch/plugin/*.java
	jar cf $(JAR) -C $(CLASSES_DIR) .
	@echo "Built: $(JAR)"

clean:
	rm -rf $(BUILD_DIR)
